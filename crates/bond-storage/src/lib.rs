#![no_std]

use serde::{Deserialize, Serialize};
use trouble_host::prelude::{BondInformation, ClientAttTableView, Identity};

pub const MAX_BONDS: usize = 4;
pub type Bonds = [Option<BondInformation>; MAX_BONDS];
const PREFIX: [u8; 4] = *b"KBB2";
const RECORD_PREFIX: [u8; 4] = *b"KBB3";
pub const CLIENT_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    BufferTooSmall,
    InvalidPayload,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientAttributes {
    schema: u32,
    length: u8,
    bytes: [u8; CLIENT_CAPACITY],
}

impl ClientAttributes {
    pub fn new(schema: u32, data: &[u8]) -> Result<Self, CodecError> {
        if data.len() > CLIENT_CAPACITY {
            return Err(CodecError::BufferTooSmall);
        }
        let table =
            ClientAttTableView::try_from_raw(data).map_err(|_| CodecError::InvalidPayload)?;
        if table.raw().len() != data.len() {
            return Err(CodecError::InvalidPayload);
        }
        let mut bytes = [0; CLIENT_CAPACITY];
        bytes[..data.len()].copy_from_slice(data);
        Ok(Self {
            schema,
            length: data.len() as u8,
            bytes,
        })
    }

    pub fn get(&self, schema: u32) -> Option<&[u8]> {
        if self.schema != schema || self.length == 0 {
            return None;
        }
        self.bytes.get(..usize::from(self.length))
    }

    fn valid(&self) -> bool {
        if self.length == 0 {
            return self.schema == 0;
        }
        self.get(self.schema).is_some_and(|data| {
            ClientAttTableView::try_from_raw(data).is_ok_and(|view| view.raw().len() == data.len())
        })
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Records {
    pub bonds: Bonds,
    pub clients: [ClientAttributes; MAX_BONDS],
}

impl Records {
    pub fn inserting(&self, bond: &BondInformation) -> Self {
        let mut next = self.clone();
        let index = insertion_index(&self.bonds, bond);
        if self.bonds[index]
            .as_ref()
            .is_none_or(|old| old.ltk != bond.ltk || !old.identity.match_identity(&bond.identity))
        {
            next.clients[index] = ClientAttributes::default();
        }
        next.bonds[index] = Some(bond.clone());
        next
    }

    pub fn client(&self, identity: &Identity, schema: u32) -> Option<ClientAttributes> {
        let index = self.bonds.iter().position(|bond| {
            bond.as_ref()
                .is_some_and(|bond| bond.identity.match_identity(identity))
        })?;
        self.clients[index].get(schema)?;
        Some(self.clients[index])
    }

    pub fn set_client(
        &mut self,
        identity: &Identity,
        schema: u32,
        data: &[u8],
    ) -> Result<bool, CodecError> {
        let Some(index) = self.bonds.iter().position(|bond| {
            bond.as_ref()
                .is_some_and(|bond| bond.identity.match_identity(identity))
        }) else {
            return Ok(false);
        };
        let attributes = ClientAttributes::new(schema, data)?;
        let changed = self.clients[index] != attributes;
        self.clients[index] = attributes;
        Ok(changed)
    }
}

pub fn encode_records<'a>(records: &Records, output: &'a mut [u8]) -> Result<&'a [u8], CodecError> {
    if output.len() < RECORD_PREFIX.len() {
        return Err(CodecError::BufferTooSmall);
    }
    output[..RECORD_PREFIX.len()].copy_from_slice(&RECORD_PREFIX);
    let length = postcard::to_slice(records, &mut output[RECORD_PREFIX.len()..])
        .map_err(|_| CodecError::BufferTooSmall)?
        .len();
    Ok(&output[..RECORD_PREFIX.len() + length])
}

pub fn decode_records(payload: &[u8]) -> Result<Records, CodecError> {
    if let Some(payload) = payload.strip_prefix(&RECORD_PREFIX) {
        let (records, remaining) = postcard::take_from_bytes::<Records>(payload)
            .map_err(|_| CodecError::InvalidPayload)?;
        if !remaining.is_empty() || !records.clients.iter().all(ClientAttributes::valid) {
            return Err(CodecError::InvalidPayload);
        }
        return Ok(records);
    }
    Ok(Records {
        bonds: decode_legacy(payload)?,
        ..Default::default()
    })
}

pub fn encode<'a>(bonds: &Bonds, output: &'a mut [u8]) -> Result<&'a [u8], CodecError> {
    if output.len() < PREFIX.len() {
        return Err(CodecError::BufferTooSmall);
    }
    output[..PREFIX.len()].copy_from_slice(&PREFIX);
    let length = postcard::to_slice(bonds, &mut output[PREFIX.len()..])
        .map_err(|_| CodecError::BufferTooSmall)?
        .len();
    Ok(&output[..PREFIX.len() + length])
}

pub fn decode(payload: &[u8]) -> Result<Bonds, CodecError> {
    decode_records(payload).map(|records| records.bonds)
}

fn decode_legacy(payload: &[u8]) -> Result<Bonds, CodecError> {
    if let Some(current) = payload.strip_prefix(&PREFIX) {
        return exact_array(current);
    }
    if payload.starts_with(b"KBB") {
        return Err(CodecError::InvalidPayload);
    }
    if let Ok(bonds) = exact_array(payload) {
        return Ok(bonds);
    }
    let (bond, remaining) = postcard::take_from_bytes::<BondInformation>(payload)
        .map_err(|_| CodecError::InvalidPayload)?;
    if !remaining.is_empty() {
        return Err(CodecError::InvalidPayload);
    }
    Ok([Some(bond), None, None, None])
}

fn exact_array(payload: &[u8]) -> Result<Bonds, CodecError> {
    let (bonds, remaining) =
        postcard::take_from_bytes::<Bonds>(payload).map_err(|_| CodecError::InvalidPayload)?;
    if !remaining.is_empty() {
        return Err(CodecError::InvalidPayload);
    }
    Ok(bonds)
}

#[derive(Clone, Debug, PartialEq)]
pub struct SavedBonds {
    pub slot: usize,
    pub generation: u32,
    pub bonds: Bonds,
    pub clients: [ClientAttributes; MAX_BONDS],
}

pub fn decode_pages(first: &[u8], second: &[u8]) -> Option<SavedBonds> {
    let mut selected: Option<SavedBonds> = None;
    for (slot, page) in [first, second].into_iter().enumerate() {
        let Ok(envelope) = kb_core::storage::decode(page) else {
            continue;
        };
        let Ok(records) = decode_records(envelope.payload) else {
            continue;
        };
        if selected.as_ref().is_none_or(|previous| {
            kb_core::storage::generation_is_newer(envelope.generation, previous.generation)
        }) {
            selected = Some(SavedBonds {
                slot,
                generation: envelope.generation,
                bonds: records.bonds,
                clients: records.clients,
            });
        }
    }
    selected
}

pub fn inserting(bonds: &Bonds, bond: &BondInformation) -> Bonds {
    let mut updated = bonds.clone();
    let index = insertion_index(bonds, bond);
    updated[index] = Some(bond.clone());
    updated
}

fn insertion_index(bonds: &Bonds, bond: &BondInformation) -> usize {
    bonds
        .iter()
        .position(|entry| {
            entry
                .as_ref()
                .is_some_and(|old| old.identity.match_identity(&bond.identity))
        })
        .or_else(|| bonds.iter().position(Option::is_none))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::num::NonZeroU128;
    use trouble_host::prelude::{
        Address, Identity, IdentityResolvingKey, LongTermKey, SecurityLevel,
    };

    fn bond(index: u8) -> BondInformation {
        BondInformation::new(
            Identity {
                addr: Address::random([index, 0xff, 0xff, 0xff, 0xff, 0xff]),
                irk: Some(IdentityResolvingKey(
                    NonZeroU128::new(u128::MAX - u128::from(index)).unwrap(),
                )),
            },
            LongTermKey(u128::MAX),
            SecurityLevel::Encrypted,
            true,
        )
    }

    #[test]
    fn four_maximum_size_bonds_round_trip_within_flash_payload_budget() {
        let bonds = core::array::from_fn(|index| Some(bond(index as u8)));
        let mut bytes = [0; 512];
        let encoded = encode(&bonds, &mut bytes).unwrap();
        assert!(encoded.len() <= 512);
        assert_eq!(decode(encoded).unwrap(), bonds);
    }

    #[test]
    fn old_single_bond_and_array_records_migrate() {
        let old = bond(1);
        let mut bytes = [0; 512];
        let encoded = postcard::to_slice(&old, &mut bytes).unwrap();
        assert_eq!(
            decode(encoded).unwrap(),
            [Some(old.clone()), None, None, None]
        );
        let bonds = [Some(old), None, Some(bond(2)), None];
        let encoded = postcard::to_slice(&bonds, &mut bytes).unwrap();
        assert_eq!(decode(encoded).unwrap(), bonds);
    }

    #[test]
    fn trailing_bytes_and_truncated_records_are_rejected() {
        let bonds = [Some(bond(1)), None, None, None];
        let mut bytes = [0; 512];
        let length = encode(&bonds, &mut bytes).unwrap().len();
        assert!(decode(&bytes[..length + 1]).is_err());
        for end in 0..length {
            assert!(
                decode(&bytes[..end]).is_err(),
                "accepted truncation at {end}"
            );
        }
    }

    #[test]
    fn unknown_version_is_not_decoded_as_a_legacy_bond() {
        assert!(decode(b"KBB4\0\0\0\0").is_err());
    }

    #[test]
    fn updating_a_peer_preserves_other_pairings() {
        let original = [Some(bond(1)), Some(bond(2)), None, None];
        let mut changed = bond(1);
        changed.ltk = LongTermKey(42);
        let updated = inserting(&original, &changed);
        assert_eq!(updated, [Some(changed), Some(bond(2)), None, None]);
    }

    #[test]
    fn full_storage_evicts_one_peer() {
        let original = core::array::from_fn(|index| Some(bond(index as u8)));
        let updated = inserting(&original, &bond(9));
        assert_eq!(updated[0], Some(bond(9)));
        assert_eq!(updated[1..], original[1..]);
    }

    #[test]
    fn valid_older_pairings_survive_an_incompatible_newer_payload() {
        let bonds = [Some(bond(1)), None, None, None];
        let mut payload = [0; 512];
        let encoded = encode(&bonds, &mut payload).unwrap();
        let mut first = [0; 528];
        let mut second = [0; 528];
        kb_core::storage::encode(encoded, 5, &mut first).unwrap();
        kb_core::storage::encode(b"KBB4new format", 6, &mut second).unwrap();
        let saved = decode_pages(&first, &second).unwrap();
        assert_eq!(saved.slot, 0);
        assert_eq!(saved.generation, 5);
        assert_eq!(saved.bonds, bonds);
    }

    #[test]
    fn generation_wrap_selects_the_last_committed_bonds() {
        let mut payload = [0; 512];
        let mut first = [0; 528];
        let mut second = [0; 528];
        let encoded = encode(&[Some(bond(1)), None, None, None], &mut payload).unwrap();
        kb_core::storage::encode(encoded, u32::MAX, &mut first).unwrap();
        let expected = [Some(bond(2)), None, None, None];
        let encoded = encode(&expected, &mut payload).unwrap();
        kb_core::storage::encode(encoded, 0, &mut second).unwrap();
        assert_eq!(decode_pages(&first, &second).unwrap().bonds, expected);
    }

    fn client_table() -> trouble_host::prelude::ClientAttTable {
        let mut builder = trouble_host::prelude::ClientAttTable::builder();
        for handle in [10, 20, 30, 40] {
            builder.push(handle, 3, false);
        }
        let mut table = builder.build();
        table.write(10, 0, &[1, 0, 0]).unwrap();
        table
    }

    #[test]
    fn all_bonds_and_full_client_tables_fit_the_flash_payload() {
        let mut records = Records::default();
        let table = client_table();
        assert_eq!(table.raw().len(), CLIENT_CAPACITY);
        for index in 0..4 {
            let bond = bond(index);
            records = records.inserting(&bond);
            assert!(records.set_client(&bond.identity, u32::MAX, table.raw()).unwrap());
        }
        let mut bytes = [0; 512];
        let payload = encode_records(&records, &mut bytes).unwrap();
        assert_eq!(decode_records(payload).unwrap(), records);
        assert!(payload.len() <= 512);
        for end in 0..payload.len() {
            assert!(decode_records(&payload[..end]).is_err());
        }
    }

    #[test]
    fn client_tables_are_bound_to_peer_identity_and_server_schema() {
        let mut records = Records::default().inserting(&bond(1)).inserting(&bond(2));
        let table = client_table();
        records
            .set_client(&bond(1).identity, 17, table.raw())
            .unwrap();
        assert_eq!(
            records.client(&bond(1).identity, 17).unwrap().get(17),
            Some(table.raw())
        );
        assert!(records.client(&bond(2).identity, 17).is_none());
        assert!(records.client(&bond(1).identity, 18).is_none());
        let mut repaired = bond(1);
        repaired.ltk = LongTermKey(1);
        assert!(
            records
                .inserting(&repaired)
                .client(&repaired.identity, 17)
                .is_none()
        );
    }

    #[test]
    fn migration_preserves_bonds_and_starts_without_client_subscriptions() {
        let bonds = [Some(bond(1)), None, None, None];
        let mut bytes = [0; 512];
        let payload = encode(&bonds, &mut bytes).unwrap();
        let migrated = decode_records(payload).unwrap();
        assert_eq!(migrated.bonds, bonds);
        assert!(
            migrated
                .clients
                .iter()
                .all(|client| *client == ClientAttributes::default())
        );
    }

    #[test]
    fn invalid_or_trailing_client_tables_are_rejected() {
        assert!(ClientAttributes::new(1, &[0xff; 32]).is_err());
        assert!(ClientAttributes::new(1, &[0; 5]).is_err());
        let mut records = Records::default().inserting(&bond(1));
        records.clients[0].length = 33;
        let mut bytes = [0; 512];
        let payload = encode_records(&records, &mut bytes).unwrap();
        assert!(decode_records(payload).is_err());
    }
}
