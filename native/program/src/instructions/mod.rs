//! Native USDC Stream Instructions
//!
//! Every instruction is a plain 1-byte tag followed by an exact-length payload. There is no
//! Anchor discriminator or serializer; a short, long, or unrecognized payload is rejected.

pub mod initialize;
pub mod release;

use pinocchio::error::ProgramError;

use crate::error::StreamError;

/// Native USDC stream instruction discriminants
///
/// 0 - Initialize
/// 1 - Release
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StreamInstruction {
    /// Create the stream PDA and move `total_amount` into its vault.
    /// Data: `[stream_id: 8][total_amount: 8][chunk_amount: 8][start_timestamp: 8][interval_seconds: 8][stream_bump: 1]`
    Initialize = 0,
    /// Release exactly one due chunk to the recipient. Permissionless; idempotent on replay.
    /// Data: `[expected_chunk_index: 8]`
    Release = 1,
}

impl TryFrom<&u8> for StreamInstruction {
    type Error = ProgramError;

    fn try_from(tag: &u8) -> Result<Self, ProgramError> {
        match tag {
            0 => Ok(Self::Initialize),
            1 => Ok(Self::Release),
            _ => Err(StreamError::InvalidInstruction.into()),
        }
    }
}

/// Read a little-endian `u64` at `offset`, rejecting out-of-range reads.
pub(crate) fn read_u64(data: &[u8], offset: usize) -> Result<u64, ProgramError> {
    let bytes: [u8; 8] = data
        .get(offset..offset.checked_add(8).ok_or(StreamError::InvalidInstruction)?)
        .ok_or(StreamError::InvalidInstruction)?
        .try_into()
        .map_err(|_| StreamError::InvalidInstruction)?;
    Ok(u64::from_le_bytes(bytes))
}

/// Read a little-endian `i64` at `offset`, rejecting out-of-range reads.
pub(crate) fn read_i64(data: &[u8], offset: usize) -> Result<i64, ProgramError> {
    let bytes: [u8; 8] = data
        .get(offset..offset.checked_add(8).ok_or(StreamError::InvalidInstruction)?)
        .ok_or(StreamError::InvalidInstruction)?
        .try_into()
        .map_err(|_| StreamError::InvalidInstruction)?;
    Ok(i64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tag_round_trips() {
        assert_eq!(
            StreamInstruction::try_from(&0),
            Ok(StreamInstruction::Initialize)
        );
        assert_eq!(
            StreamInstruction::try_from(&1),
            Ok(StreamInstruction::Release)
        );
    }

    #[test]
    fn unknown_tag_is_rejected() {
        assert!(StreamInstruction::try_from(&2).is_err());
        assert!(StreamInstruction::try_from(&99).is_err());
    }

    #[test]
    fn out_of_range_reads_are_rejected() {
        assert!(read_u64(&[1, 2, 3], 0).is_err());
        assert!(read_i64(&[0; 8], 1).is_err());
        assert_eq!(read_u64(&7u64.to_le_bytes(), 0), Ok(7));
    }
}
