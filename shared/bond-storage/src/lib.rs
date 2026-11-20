#![no_std]

use serde::{Deserialize, Serialize};
use trouble_host::prelude::{BondInformation, ClientAttTableView, Identity};

pub const MAX_BONDS: usize = 7;
pub type Bonds = [Option<BondInformation>; MAX_BONDS];
type LegacyBonds = [Option<BondInformation>; 4];
const LEGACY_PREFIX: [u8; 4] = *b"KBB2";
const LEGACY_RECORD_PREFIX: [u8; 4] = *b"KBB3";
const PREFIX: [u8; 4] = *b"KBB4";
const RECORD_PREFIX: [u8; 4] = *b"KBB5";
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

    pub fn client_peer(&self, schema: u32) -> Option<Identity> {
        self.bonds
            .iter()
            .zip(&self.clients)
            .find_map(|(bond, attributes)| {
                attributes.get(schema)?;
                Some(bond.as_ref()?.identity)
            })
    }

    pub fn allows_client_peer(&self, schema: u32, identity: &Identity, known: bool) -> bool {
        if let Some(peer) = self.client_peer(schema) {
            return peer.match_identity(identity);
        }
        !known
            && !self
                .bonds
                .iter()
                .flatten()
                .any(|bond| bond.identity.match_identity(identity))
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
    let records = if let Some(payload) = payload.strip_prefix(&RECORD_PREFIX) {
        exact(payload)?
    } else if let Some(payload) = payload.strip_prefix(&LEGACY_RECORD_PREFIX) {
        let (bonds, clients): (LegacyBonds, [ClientAttributes; 4]) = exact(payload)?;
        let mut records = Records {
            bonds: expand(bonds),
            ..Default::default()
        };
        records.clients[..clients.len()].copy_from_slice(&clients);
        records
    } else {
        Records {
            bonds: decode_without_clients(payload)?,
            ..Default::default()
        }
    };
    if !records.clients.iter().all(ClientAttributes::valid) {
        return Err(CodecError::InvalidPayload);
    }
    Ok(records)
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

fn decode_without_clients(payload: &[u8]) -> Result<Bonds, CodecError> {
    if let Some(current) = payload.strip_prefix(&PREFIX) {
        return exact(current);
    }
    if let Some(legacy) = payload.strip_prefix(&LEGACY_PREFIX) {
        return exact(legacy).map(expand);
    }
    if payload.starts_with(b"KBB") {
        return Err(CodecError::InvalidPayload);
    }
    if let Ok(bonds) = exact(payload) {
        return Ok(expand(bonds));
    }
    let mut bonds: Bonds = Default::default();
    bonds[0] = Some(exact(payload)?);
    Ok(bonds)
}

fn expand(legacy: LegacyBonds) -> Bonds {
    let mut legacy = legacy.into_iter();
    core::array::from_fn(|_| legacy.next().flatten())
}

fn exact<'a, T: Deserialize<'a>>(payload: &'a [u8]) -> Result<T, CodecError> {
    let (value, remaining) =
        postcard::take_from_bytes::<T>(payload).map_err(|_| CodecError::InvalidPayload)?;
    if !remaining.is_empty() {
        return Err(CodecError::InvalidPayload);
    }
    Ok(value)
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
    use kb_core::storage::MAX_PAYLOAD_LEN;
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
    fn seven_maximum_size_bonds_round_trip_within_flash_payload_budget() {
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
            expand([Some(old.clone()), None, None, None])
        );
        let bonds = [Some(old), None, Some(bond(2)), None];
        let encoded = postcard::to_slice(&bonds, &mut bytes).unwrap();
        assert_eq!(decode(encoded).unwrap(), expand(bonds.clone()));
        bytes[..4].copy_from_slice(&LEGACY_PREFIX);
        let len = postcard::to_slice(&bonds, &mut bytes[4..]).unwrap().len() + 4;
        assert_eq!(decode(&bytes[..len]).unwrap(), expand(bonds));
    }

    #[test]
    fn trailing_bytes_and_truncated_records_are_rejected() {
        let bonds = expand([Some(bond(1)), None, None, None]);
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
        assert!(decode(b"KBB6\0\0\0\0").is_err());
    }

    #[test]
    fn updating_a_peer_preserves_other_pairings() {
        let original = expand([Some(bond(1)), Some(bond(2)), None, None]);
        let mut changed = bond(1);
        changed.ltk = LongTermKey(42);
        let updated = inserting(&original, &changed);
        assert_eq!(updated, expand([Some(changed), Some(bond(2)), None, None]));
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
        let bonds = expand([Some(bond(1)), None, None, None]);
        let mut payload = [0; 512];
        let encoded = encode(&bonds, &mut payload).unwrap();
        let mut first = [0; 528];
        let mut second = [0; 528];
        kb_core::storage::encode(encoded, 5, &mut first).unwrap();
        kb_core::storage::encode(b"KBB6new format", 6, &mut second).unwrap();
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
        let original = expand([Some(bond(1)), None, None, None]);
        let encoded = encode(&original, &mut payload).unwrap();
        kb_core::storage::encode(encoded, u32::MAX, &mut first).unwrap();
        let expected = expand([Some(bond(2)), None, None, None]);
        let encoded = encode(&expected, &mut payload).unwrap();
        kb_core::storage::encode(encoded, 0, &mut second).unwrap();
        assert_eq!(decode_pages(&first, &second).unwrap().bonds, expected);
    }

    #[test]
    fn committed_empty_record_clears_bonds_before_old_page_is_erased() {
        let mut payload = [0; 512];
        let mut first = [0xff; 528];
        let mut second = [0xff; 528];
        let records = Records::default().inserting(&bond(1));
        let encoded = encode_records(&records, &mut payload).unwrap();
        kb_core::storage::encode(encoded, 7, &mut first).unwrap();
        let cleared = Records::default();
        let encoded = encode_records(&cleared, &mut payload).unwrap();
        kb_core::storage::encode(encoded, 8, &mut second).unwrap();

        let selected = decode_pages(&first, &second).unwrap();
        assert_eq!(selected.bonds, cleared.bonds);
        assert_eq!(selected.clients, cleared.clients);

        second[..kb_core::storage::COMMIT_LEN].fill(0xff);
        assert_eq!(decode_pages(&first, &second).unwrap().bonds, records.bonds);
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
        for index in 0..MAX_BONDS {
            let bond = bond(index as u8);
            records = records.inserting(&bond);
            assert!(
                records
                    .set_client(&bond.identity, u32::MAX, table.raw())
                    .unwrap()
            );
        }
        let mut bytes = [0; MAX_PAYLOAD_LEN];
        let payload = encode_records(&records, &mut bytes).unwrap();
        assert_eq!(decode_records(payload).unwrap(), records);
        for end in 0..payload.len() {
            assert!(decode_records(&payload[..end]).is_err());
        }
    }

    #[test]
    fn numbered_clients_keep_independent_peers_and_reject_other_bonds() {
        const LEGACY_HID: u32 = 0x5443_4849;
        let mut records = Records::default();
        let table = client_table();
        for slot in 0..5 {
            let host = bond(slot);
            let schema = LEGACY_HID + u32::from(slot);
            assert!(records.allows_client_peer(schema, &host.identity, false));
            if slot > 0 {
                assert!(!records.allows_client_peer(schema, &bond(0).identity, true));
                assert!(!records.allows_client_peer(schema, &bond(0).identity, false));
            }
            records = records.inserting(&host);
            records
                .set_client(&host.identity, schema, table.raw())
                .unwrap();
        }
        let mut bytes = [0; kb_core::storage::MAX_PAYLOAD_LEN];
        let reloaded = decode_records(encode_records(&records, &mut bytes).unwrap()).unwrap();
        for slot in 0..5 {
            let schema = LEGACY_HID + u32::from(slot);
            for host in 0..5 {
                assert_eq!(
                    reloaded.allows_client_peer(schema, &bond(host).identity, true),
                    host == slot
                );
            }
            assert!(!reloaded.allows_client_peer(schema, &bond(7).identity, false));
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
        assert_eq!(records.client_peer(17), Some(bond(1).identity));
        assert_eq!(records.client_peer(18), None);
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
        let bonds = expand([Some(bond(1)), None, None, None]);
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
    fn four_peer_records_keep_client_attributes_when_migrated() {
        let old = [Some(bond(1)), None, Some(bond(2)), None];
        let table = ClientAttributes::new(17, client_table().raw()).unwrap();
        let clients = [
            table,
            ClientAttributes::default(),
            table,
            ClientAttributes::default(),
        ];
        let mut bytes = [0; MAX_PAYLOAD_LEN];
        bytes[..4].copy_from_slice(&LEGACY_RECORD_PREFIX);
        let len = postcard::to_slice(&(old.clone(), clients), &mut bytes[4..])
            .unwrap()
            .len()
            + 4;
        let migrated = decode_records(&bytes[..len]).unwrap();
        assert_eq!(migrated.bonds, expand(old));
        assert_eq!(migrated.clients[..4], clients);
        assert_eq!(migrated.clients[4..], [ClientAttributes::default(); 3]);
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
