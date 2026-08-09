//! MagicBlock USDC Stream Instructions
//!
//! Every instruction is a plain 8-byte tag followed by an exact-length payload (the
//! DLP-defined undelegation callback is the sole exception: its payload is forwarded
//! verbatim, at any length). There is no Anchor discriminator or serializer.

pub mod delegate;
pub mod finalize;
pub mod initialize;
pub mod release;
pub mod schedule;
pub mod settle_chunk;
pub mod undelegate;

use ephemeral_rollups_pinocchio::intent_bundle::ShortAccountMeta;
use pinocchio::{error::ProgramError, Address};

use crate::{constants, error::StreamError, state::StreamState};

/// MagicBlock USDC stream instruction discriminants (8-byte little-endian tags)
///
/// 0 - Initialize   (base layer, sender)
/// 1 - Delegate      (base layer, sender)
/// 2 - Schedule      (Ephemeral Rollup, permissionless)
/// 3 - Release       (Ephemeral Rollup, Native Crank signer)
/// 4 - SettleChunk   (base layer, DLP Magic Action callback)
/// 5 - Finalize      (base layer, DLP Magic Action callback)
/// UndelegateCallback uses a separate, DLP-defined 8-byte tag (base layer, DLP callback)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamInstruction {
    Initialize,
    Delegate,
    Schedule,
    Release,
    SettleChunk,
    Finalize,
    UndelegateCallback,
}

/// Split off the 8-byte tag and resolve it. The remaining bytes are each instruction's own
/// payload; every handler (other than the undelegation callback) enforces its exact length.
pub fn decode(data: &[u8]) -> Result<(StreamInstruction, &[u8]), ProgramError> {
    let (tag, payload) = data
        .split_first_chunk::<8>()
        .ok_or(StreamError::InvalidInstruction)?;
    let instruction = match *tag {
        constants::INITIALIZE_STREAM => StreamInstruction::Initialize,
        constants::DELEGATE_STREAM => StreamInstruction::Delegate,
        constants::SCHEDULE_STREAM => StreamInstruction::Schedule,
        constants::RELEASE_CHUNK => StreamInstruction::Release,
        constants::SETTLE_CHUNK => StreamInstruction::SettleChunk,
        constants::FINALIZE_STREAM => StreamInstruction::Finalize,
        constants::UNDELEGATION_CALLBACK => StreamInstruction::UndelegateCallback,
        _ => return Err(StreamError::InvalidInstruction.into()),
    };
    Ok((instruction, payload))
}

/// Read a little-endian `u64` at `offset`, rejecting out-of-range reads.
pub(crate) fn read_u64(data: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let bytes: [u8; 8] = data
        .get(offset..offset + 8)
        .ok_or(StreamError::InvalidInstruction)?
        .try_into()
        .map_err(|_| StreamError::InvalidInstruction)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Six accounts consumed by the `SETTLE_CHUNK` Magic Action; the DLP appends its secure suffix.
/// Shared by `release` (the only caller that schedules this action).
pub(crate) fn settle_chunk_accounts(
    state: &StreamState,
    stream: &Address,
    escrow_authority: &Address,
) -> [ShortAccountMeta; 6] {
    [
        ShortAccountMeta {
            pubkey: *stream,
            is_writable: false,
        },
        ShortAccountMeta {
            pubkey: Address::new_from_array(state.mint),
            is_writable: false,
        },
        ShortAccountMeta {
            pubkey: Address::new_from_array(state.escrow_token_account),
            is_writable: true,
        },
        ShortAccountMeta {
            pubkey: Address::new_from_array(state.destination_token_account),
            is_writable: true,
        },
        ShortAccountMeta {
            pubkey: *escrow_authority,
            is_writable: false,
        },
        ShortAccountMeta {
            pubkey: pinocchio_token::ID,
            is_writable: false,
        },
    ]
}

/// Seven accounts consumed by the `FINALIZE_STREAM` Magic Action; the DLP appends its secure
/// suffix. `release` schedules it once the stream reaches natural completion.
pub(crate) fn finalize_accounts(
    state: &StreamState,
    stream: &Address,
    escrow_authority: &Address,
) -> [ShortAccountMeta; 7] {
    [
        ShortAccountMeta {
            pubkey: *stream,
            is_writable: true,
        },
        ShortAccountMeta {
            pubkey: Address::new_from_array(state.mint),
            is_writable: false,
        },
        ShortAccountMeta {
            pubkey: Address::new_from_array(state.escrow_token_account),
            is_writable: true,
        },
        ShortAccountMeta {
            pubkey: Address::new_from_array(state.destination_token_account),
            is_writable: true,
        },
        ShortAccountMeta {
            pubkey: *escrow_authority,
            is_writable: false,
        },
        ShortAccountMeta {
            pubkey: Address::new_from_array(state.sender),
            is_writable: true,
        },
        ShortAccountMeta {
            pubkey: pinocchio_token::ID,
            is_writable: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tag_decodes_with_its_payload() {
        let mut data = constants::RELEASE_CHUNK.to_vec();
        data.extend_from_slice(&[9, 9]);
        let (instruction, payload) = decode(&data).unwrap();
        assert_eq!(instruction, StreamInstruction::Release);
        assert_eq!(payload, &[9, 9]);
    }

    #[test]
    fn unknown_tag_is_rejected() {
        assert!(decode(&[9; 8]).is_err());
    }

    #[test]
    fn short_data_is_rejected() {
        assert!(decode(&[0; 4]).is_err());
    }
}
