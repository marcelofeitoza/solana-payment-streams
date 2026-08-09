//! Stream account state.
//!
//! Exact state committed between Surfpool and the local Ephemeral Rollup.

use ephemeral_rollups_pinocchio::{consts::DELEGATION_PROGRAM_ID, pda::delegation_record_pda_from_delegated_account};
use pinocchio::{error::ProgramError, AccountView, Address};

use crate::{error::StreamError, pda};

pub const STREAM_DISCRIMINATOR: [u8; 8] = *b"USDCSTRM";
pub const STREAM_VERSION: u8 = 1;
pub const STREAM_STATE_LEN: usize = 277;

const SENDER_OFFSET: usize = 9;
const RECIPIENT_OFFSET: usize = 41;
const MINT_OFFSET: usize = 73;
const SOURCE_OFFSET: usize = 105;
const DESTINATION_OFFSET: usize = 137;
const ESCROW_TOKEN_OFFSET: usize = 169;
const VALIDATOR_OFFSET: usize = 201;
const TOTAL_OFFSET: usize = 233;
const CHUNK_OFFSET: usize = 241;
const SENT_OFFSET: usize = 249;
const INTERVAL_OFFSET: usize = 257;
const ACTIVE_OFFSET: usize = 265;
const BUMP_OFFSET: usize = 266;
const ESCROW_BUMP_OFFSET: usize = 267;
const SCHEDULED_OFFSET: usize = 268;
const TASK_ID_OFFSET: usize = 269;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamState {
    pub sender: [u8; 32],
    pub recipient: [u8; 32],
    pub mint: [u8; 32],
    pub source_token_account: [u8; 32],
    pub destination_token_account: [u8; 32],
    /// Program-controlled token account holding funds between releases.
    pub escrow_token_account: [u8; 32],
    pub total_amount: u64,
    pub chunk_amount: u64,
    pub sent_amount: u64,
    pub validator: [u8; 32],
    pub interval_ms: u64,
    pub active: bool,
    pub bump: u8,
    pub escrow_bump: u8,
    pub scheduled: bool,
    pub task_id: i64,
}

impl StreamState {
    /// Decode state while rejecting unknown versions and non-canonical booleans.
    pub fn unpack(data: &[u8]) -> Option<Self> {
        if data.len() != STREAM_STATE_LEN
            || data[..8] != STREAM_DISCRIMINATOR
            || data[8] != STREAM_VERSION
        {
            return None;
        }

        Some(Self {
            sender: read_address(data, SENDER_OFFSET),
            recipient: read_address(data, RECIPIENT_OFFSET),
            mint: read_address(data, MINT_OFFSET),
            source_token_account: read_address(data, SOURCE_OFFSET),
            destination_token_account: read_address(data, DESTINATION_OFFSET),
            escrow_token_account: read_address(data, ESCROW_TOKEN_OFFSET),
            validator: read_address(data, VALIDATOR_OFFSET),
            total_amount: read_u64(data, TOTAL_OFFSET),
            chunk_amount: read_u64(data, CHUNK_OFFSET),
            sent_amount: read_u64(data, SENT_OFFSET),
            interval_ms: read_u64(data, INTERVAL_OFFSET),
            active: read_bool(data[ACTIVE_OFFSET])?,
            bump: data[BUMP_OFFSET],
            escrow_bump: data[ESCROW_BUMP_OFFSET],
            scheduled: read_bool(data[SCHEDULED_OFFSET])?,
            task_id: read_i64(data, TASK_ID_OFFSET),
        })
    }

    /// Encode the canonical byte representation.
    pub fn pack(&self, data: &mut [u8]) -> Option<()> {
        if data.len() != STREAM_STATE_LEN {
            return None;
        }
        data.fill(0);
        data[..8].copy_from_slice(&STREAM_DISCRIMINATOR);
        data[8] = STREAM_VERSION;
        write_address(data, SENDER_OFFSET, &self.sender);
        write_address(data, RECIPIENT_OFFSET, &self.recipient);
        write_address(data, MINT_OFFSET, &self.mint);
        write_address(data, SOURCE_OFFSET, &self.source_token_account);
        write_address(data, DESTINATION_OFFSET, &self.destination_token_account);
        write_address(data, ESCROW_TOKEN_OFFSET, &self.escrow_token_account);
        write_address(data, VALIDATOR_OFFSET, &self.validator);
        write_u64(data, TOTAL_OFFSET, self.total_amount);
        write_u64(data, CHUNK_OFFSET, self.chunk_amount);
        write_u64(data, SENT_OFFSET, self.sent_amount);
        write_u64(data, INTERVAL_OFFSET, self.interval_ms);
        data[ACTIVE_OFFSET] = u8::from(self.active);
        data[BUMP_OFFSET] = self.bump;
        data[ESCROW_BUMP_OFFSET] = self.escrow_bump;
        data[SCHEDULED_OFFSET] = u8::from(self.scheduled);
        data[TASK_ID_OFFSET..TASK_ID_OFFSET + 8].copy_from_slice(&self.task_id.to_le_bytes());
        Some(())
    }

    /// Load and decode the stream state stored in `account`.
    pub fn load(account: &AccountView) -> Result<Self, ProgramError> {
        if account.data_len() != STREAM_STATE_LEN {
            return Err(StreamError::InvalidState.into());
        }
        let data = account.try_borrow()?;
        Self::unpack(&data).ok_or_else(|| StreamError::InvalidState.into())
    }

    /// Persist this stream state into `account`.
    pub fn store(&self, account: &AccountView) -> Result<(), ProgramError> {
        let mut data = account.try_borrow_mut()?;
        self.pack(&mut data).ok_or_else(|| StreamError::InvalidState.into())
    }

    /// Re-derive this stream's canonical PDA and its escrow-authority PDA, and confirm
    /// `stream`/`escrow_authority` match them exactly.
    pub fn validate_pdas(
        &self,
        program_id: &Address,
        stream: &AccountView,
        escrow_authority: &AccountView,
    ) -> Result<(), ProgramError> {
        if stream.address() != &pda::stream_address(program_id, self)? {
            return Err(StreamError::InvalidPda.into());
        }
        if escrow_authority.address()
            != &pda::escrow_authority_address(program_id, stream.address(), self.escrow_bump)?
        {
            return Err(StreamError::InvalidPda.into());
        }
        Ok(())
    }

    /// Verify both the canonical delegation-record PDA and its recorded validator authority.
    pub fn validate_delegation(
        &self,
        stream: &AccountView,
        delegation_record: &AccountView,
    ) -> Result<(), ProgramError> {
        let expected = delegation_record_pda_from_delegated_account(stream.address());
        if delegation_record.address() != &expected
            || !delegation_record.owned_by(&DELEGATION_PROGRAM_ID)
        {
            return Err(StreamError::InvalidValidator.into());
        }
        let data = delegation_record.try_borrow()?;
        if data.len() < 40 || data[8..40] != self.validator {
            return Err(StreamError::InvalidValidator.into());
        }
        Ok(())
    }

    /// Amount not yet authorized by successful crank iterations.
    pub fn remaining(&self) -> Option<u64> {
        self.total_amount.checked_sub(self.sent_amount)
    }

    /// Ceiling division for the exact number of remaining crank calls.
    pub fn remaining_iterations(&self) -> Option<u64> {
        let remaining = self.remaining()?;
        if self.chunk_amount == 0 {
            return None;
        }
        remaining
            .checked_add(self.chunk_amount.checked_sub(1)?)
            .map(|value| value / self.chunk_amount)
    }
}

fn read_bool(value: u8) -> Option<bool> {
    match value {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn read_address(data: &[u8], offset: usize) -> [u8; 32] {
    let mut value = [0_u8; 32];
    value.copy_from_slice(&data[offset..offset + 32]);
    value
}

fn read_u64(data: &[u8], offset: usize) -> u64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(value)
}

fn read_i64(data: &[u8], offset: usize) -> i64 {
    let mut value = [0_u8; 8];
    value.copy_from_slice(&data[offset..offset + 8]);
    i64::from_le_bytes(value)
}

fn write_address(data: &mut [u8], offset: usize, value: &[u8; 32]) {
    data[offset..offset + 32].copy_from_slice(value);
}

fn write_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_round_trip_and_iteration_math_are_exact() {
        let state = StreamState {
            sender: [1; 32],
            recipient: [2; 32],
            mint: [3; 32],
            source_token_account: [4; 32],
            destination_token_account: [5; 32],
            escrow_token_account: [7; 32],
            total_amount: crate::constants::DEFAULT_TOTAL_AMOUNT,
            chunk_amount: crate::constants::DEFAULT_CHUNK_AMOUNT,
            sent_amount: 0,
            validator: [6; 32],
            interval_ms: 10,
            active: true,
            bump: 250,
            escrow_bump: 249,
            scheduled: false,
            task_id: crate::constants::CRANK_TASK_ID,
        };
        let mut bytes = [0_u8; STREAM_STATE_LEN];
        state.pack(&mut bytes).unwrap();
        assert_eq!(StreamState::unpack(&bytes), Some(state));
        assert_eq!(
            state.remaining_iterations(),
            Some(crate::constants::DEFAULT_ITERATIONS)
        );
    }
}
