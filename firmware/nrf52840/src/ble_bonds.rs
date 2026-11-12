use core::sync::atomic::{AtomicBool, Ordering};
use kb_core::storage::{COMMIT_LEN, MAX_ENCODED_LEN, MAX_PAYLOAD_LEN, encode};
use nrf_mpsl::{Flash, FlashError};
use trouble_host::prelude::BondInformation;

static STORAGE_FAILED: AtomicBool = AtomicBool::new(false);

pub fn storage_failed() -> bool {
    STORAGE_FAILED.load(Ordering::Relaxed)
}

const PAGES: [u32; 2] = [0xf2000, 0xf3000];

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
    bonds: [Option<BondInformation>; 4],
}

impl<'d> BondStore<'d> {
    pub fn new(flash: Flash<'d>) -> Self {
        Self {
            flash,
            slot: None,
            generation: 0,
            bonds: core::array::from_fn(|_| None),
        }
    }

    pub fn load(&mut self) -> Result<Option<BondInformation>, Error> {
        Ok(self.load_all()?.into_iter().flatten().next())
    }

    pub fn load_all(&mut self) -> Result<[Option<BondInformation>; 4], Error> {
        let mut pages = [[0; MAX_ENCODED_LEN]; 2];
        self.flash.read(PAGES[0], &mut pages[0])?;
        self.flash.read(PAGES[1], &mut pages[1])?;
        let Some(selected) = kb_bond_storage::decode_pages(&pages[0], &pages[1]) else {
            return Ok(self.bonds.clone());
        };
        self.bonds = selected.bonds;
        self.slot = Some(selected.slot);
        self.generation = selected.generation;
        Ok(self.bonds.clone())
    }

    pub async fn remember(&mut self, bond: &BondInformation) {
        let pending = kb_bond_storage::inserting(&self.bonds, bond);
        let failed = self.save(bond).await.is_err();
        if failed {
            self.bonds = pending;
        }
        STORAGE_FAILED.store(failed, Ordering::Relaxed);
    }

    pub async fn forget(&mut self) {
        STORAGE_FAILED.store(self.clear().await.is_err(), Ordering::Relaxed);
    }

    pub async fn save(&mut self, bond: &BondInformation) -> Result<(), Error> {
        let bonds = kb_bond_storage::inserting(&self.bonds, bond);
        let mut payload = [0; MAX_PAYLOAD_LEN];
        let payload =
            kb_bond_storage::encode(&bonds, &mut payload).map_err(|_| Error::Serialization)?;
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
        self.bonds = bonds;
        Ok(())
    }

    pub async fn clear(&mut self) -> Result<(), Error> {
        for page in PAGES {
            self.flash.erase(page, page + 4096).await?;
        }
        self.slot = None;
        self.generation = 0;
        self.bonds = core::array::from_fn(|_| None);
        Ok(())
    }
}
