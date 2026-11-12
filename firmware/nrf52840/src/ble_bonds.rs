use kb_core::storage::{COMMIT_LEN, MAX_ENCODED_LEN, MAX_PAYLOAD_LEN, encode, select_newest};
use nrf_mpsl::{Flash, FlashError};
use trouble_host::prelude::BondInformation;

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
        let Some(selected) = select_newest(&pages[0], &pages[1]) else {
            return Ok(self.bonds.clone());
        };
        self.bonds = postcard::from_bytes(selected.envelope.payload)
            .or_else(|_| {
                postcard::from_bytes::<BondInformation>(selected.envelope.payload)
                    .map(|bond| [Some(bond), None, None, None])
            })
            .map_err(|_| Error::Serialization)?;
        self.slot = Some(selected.slot);
        self.generation = selected.envelope.generation;
        Ok(self.bonds.clone())
    }

    pub async fn save(&mut self, bond: &BondInformation) -> Result<(), Error> {
        let mut bonds = self.bonds.clone();
        let index = bonds
            .iter()
            .position(|entry| {
                entry
                    .as_ref()
                    .is_some_and(|old| old.identity == bond.identity)
            })
            .or_else(|| bonds.iter().position(Option::is_none))
            .unwrap_or(0);
        bonds[index] = Some(bond.clone());
        let mut payload = [0; MAX_PAYLOAD_LEN];
        let payload = postcard::to_slice(&bonds, &mut payload).map_err(|_| Error::Serialization)?;
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
