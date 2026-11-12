#![no_std]

use trouble_host::prelude::BondInformation;

pub const MAX_BONDS: usize = 4;
pub type Bonds = [Option<BondInformation>; MAX_BONDS];
const PREFIX: [u8; 4] = *b"KBB2";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    BufferTooSmall,
    InvalidPayload,
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
}

pub fn decode_pages(first: &[u8], second: &[u8]) -> Option<SavedBonds> {
    let mut selected: Option<SavedBonds> = None;
    for (slot, page) in [first, second].into_iter().enumerate() {
        let Ok(envelope) = kb_core::storage::decode(page) else {
            continue;
        };
        let Ok(bonds) = decode(envelope.payload) else {
            continue;
        };
        if selected.as_ref().is_none_or(|previous| {
            kb_core::storage::generation_is_newer(envelope.generation, previous.generation)
        }) {
            selected = Some(SavedBonds {
                slot,
                generation: envelope.generation,
                bonds,
            });
        }
    }
    selected
}

pub fn inserting(bonds: &Bonds, bond: &BondInformation) -> Bonds {
    let mut updated = bonds.clone();
    let index = updated
        .iter()
        .position(|entry| {
            entry
                .as_ref()
                .is_some_and(|old| old.identity == bond.identity)
        })
        .or_else(|| updated.iter().position(Option::is_none))
        .unwrap_or(0);
    updated[index] = Some(bond.clone());
    updated
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
                irk: Some(IdentityResolvingKey(NonZeroU128::new(u128::MAX).unwrap())),
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
        assert!(decode(b"KBB3\0\0\0\0").is_err());
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
        kb_core::storage::encode(b"KBB3new format", 6, &mut second).unwrap();
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
}
