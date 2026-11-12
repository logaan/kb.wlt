use core::sync::atomic::{AtomicBool, Ordering};
use embassy_sync::blocking_mutex::raw::RawMutex;
use kb_bond_storage::{ClientAttributes, Records};
use kb_core::storage::{COMMIT_LEN, MAX_ENCODED_LEN, MAX_PAYLOAD_LEN, encode};
use nrf_mpsl::{Flash, FlashError};
use trouble_host::prelude::{
    AttributeServer, BondInformation, ClientAttTableView, Connection, Identity, PacketPool,
};

static STORAGE_FAILED: AtomicBool = AtomicBool::new(false);

pub fn storage_failed() -> bool {
    STORAGE_FAILED.load(Ordering::Relaxed)
}

pub const HID_SCHEMA: u32 = 0x4849_4401;
pub const PAINTBRUSH_LINK_SCHEMA: u32 = 0x5042_0001;
pub const TOUCAN_LINK_SCHEMA: u32 = 0x5443_0001;

const PAGES: [u32; 2] = [0xf2000, 0xf3000];
const CONFIG_PAGES: [u32; 2] = [0xf0000, 0xf1000];

#[repr(align(4))]
struct AlignedBuffer([u8; MAX_ENCODED_LEN]);

#[derive(Debug)]
pub enum Error {
    Flash(FlashError),
    Serialization,
}

impl From<FlashError> for Error {
    fn from(error: FlashError) -> Self {
        Self::Flash(error)
    }
}

pub struct BondStore<'d> {
    flash: Flash<'d>,
    slot: Option<usize>,
    generation: u32,
    records: Records,
    records_loaded: bool,
    config_slot: Option<usize>,
    config_generation: u32,
}

impl<'d> BondStore<'d> {
    pub fn new(flash: Flash<'d>) -> Self {
        Self {
            flash,
            slot: None,
            generation: 0,
            records: Records::default(),
            records_loaded: false,
            config_slot: None,
            config_generation: 0,
        }
    }

    pub fn load(&mut self) -> Result<Option<BondInformation>, Error> {
        Ok(self.load_all()?.into_iter().flatten().next())
    }

    pub fn load_all(&mut self) -> Result<[Option<BondInformation>; 4], Error> {
        if !self.records_loaded {
            let mut pages = [[0; MAX_ENCODED_LEN]; 2];
            self.flash.read(PAGES[0], &mut pages[0])?;
            self.flash.read(PAGES[1], &mut pages[1])?;
            if let Some(selected) = kb_bond_storage::decode_pages(&pages[0], &pages[1]) {
                self.records = Records {
                    bonds: selected.bonds,
                    clients: selected.clients,
                };
                self.slot = Some(selected.slot);
                self.generation = selected.generation;
            }
            self.records_loaded = true;
        }
        Ok(self.records.bonds.clone())
    }

    pub fn load_config(&mut self, output: &mut [u8]) -> Result<Option<usize>, Error> {
        let mut pages = [[0; MAX_ENCODED_LEN]; 2];
        self.flash.read(CONFIG_PAGES[0], &mut pages[0])?;
        self.flash.read(CONFIG_PAGES[1], &mut pages[1])?;
        let Some(selected) = kb_core::storage::select_newest(&pages[0], &pages[1]) else {
            return Ok(None);
        };
        let payload = selected.envelope.payload;
        if output.len() < payload.len() {
            return Err(Error::Serialization);
        }
        output[..payload.len()].copy_from_slice(payload);
        self.config_slot = Some(selected.slot);
        self.config_generation = selected.envelope.generation;
        Ok(Some(payload.len()))
    }

    pub async fn save_config(&mut self, payload: &[u8]) -> Result<(), Error> {
        let generation = self.config_generation.wrapping_add(1);
        let mut encoded = AlignedBuffer([0; MAX_ENCODED_LEN]);
        let len = encode(payload, generation, &mut encoded.0).map_err(|_| Error::Serialization)?;
        let slot = self.config_slot.map_or(0, |slot| slot ^ 1);
        self.flash
            .erase(CONFIG_PAGES[slot], CONFIG_PAGES[slot] + 4096)
            .await?;
        self.flash
            .write(
                CONFIG_PAGES[slot] + COMMIT_LEN as u32,
                &encoded.0[COMMIT_LEN..len],
            )
            .await?;
        self.flash
            .write(CONFIG_PAGES[slot], &encoded.0[..COMMIT_LEN])
            .await?;
        self.config_slot = Some(slot);
        self.config_generation = generation;
        Ok(())
    }

    pub async fn remember(&mut self, bond: &BondInformation) {
        self.persist_best_effort(self.records.inserting(bond)).await;
    }

    pub fn client(&self, identity: &Identity, schema: u32) -> Option<ClientAttributes> {
        self.records.client(identity, schema)
    }

    pub fn restore_client_table<M: RawMutex, P: PacketPool, const AT: usize, const CN: usize>(
        &self,
        server: &AttributeServer<'_, M, P, AT, CN>,
        connection: &Connection<'_, P>,
        schema: u32,
    ) {
        if !connection.is_bonded_peer() {
            return;
        }
        if let Some(attributes) = self.client(&connection.peer_identity(), schema) {
            if let Some(data) = attributes.get(schema) {
                if let Ok(view) = ClientAttTableView::try_from_raw(data) {
                    server.set_client_att_table(connection, &view);
                }
            }
        }
    }

    pub async fn remember_client_table<
        M: RawMutex,
        P: PacketPool,
        const AT: usize,
        const CN: usize,
    >(
        &mut self,
        server: &AttributeServer<'_, M, P, AT, CN>,
        connection: &Connection<'_, P>,
        schema: u32,
    ) {
        if connection.is_bonded_peer() {
            if let Some(table) = server.get_client_att_table(connection) {
                self.remember_client(&connection.peer_identity(), schema, table.raw())
                    .await;
            }
        }
    }

    pub async fn remember_client(&mut self, identity: &Identity, schema: u32, data: &[u8]) {
        let mut pending = self.records.clone();
        match pending.set_client(identity, schema, data) {
            Ok(true) => self.persist_best_effort(pending).await,
            Ok(false) if storage_failed() => self.persist_best_effort(pending).await,
            Ok(false) => {}
            Err(_) => STORAGE_FAILED.store(true, Ordering::Relaxed),
        }
    }

    async fn persist_best_effort(&mut self, pending: Records) {
        self.records = pending;
        STORAGE_FAILED.store(true, Ordering::Relaxed);
        let failed = self.persist().await.is_err();
        STORAGE_FAILED.store(failed, Ordering::Relaxed);
    }

    pub async fn forget(&mut self) {
        let loaded = self.load_all().is_ok();
        self.records = Records::default();
        self.records_loaded = true;
        STORAGE_FAILED.store(true, Ordering::Relaxed);
        if loaded {
            STORAGE_FAILED.store(self.clear().await.is_err(), Ordering::Relaxed);
        }
    }

    async fn persist(&mut self) -> Result<(), Error> {
        let mut payload = [0; MAX_PAYLOAD_LEN];
        let payload = kb_bond_storage::encode_records(&self.records, &mut payload)
            .map_err(|_| Error::Serialization)?;
        let generation = self.generation.wrapping_add(1);
        let mut encoded = AlignedBuffer([0; MAX_ENCODED_LEN]);
        let len = encode(payload, generation, &mut encoded.0).map_err(|_| Error::Serialization)?;
        let slot = self.slot.map_or(0, |slot| slot ^ 1);
        self.flash.erase(PAGES[slot], PAGES[slot] + 4096).await?;
        self.flash
            .write(PAGES[slot] + COMMIT_LEN as u32, &encoded.0[COMMIT_LEN..len])
            .await?;
        self.flash
            .write(PAGES[slot], &encoded.0[..COMMIT_LEN])
            .await?;
        self.slot = Some(slot);
        self.generation = generation;
        Ok(())
    }

    async fn clear(&mut self) -> Result<(), Error> {
        let stale = PAGES[self.slot.unwrap_or(1)];
        self.persist().await?;
        self.flash.erase(stale, stale + 4096).await?;
        Ok(())
    }
}
