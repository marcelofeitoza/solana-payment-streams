//! Stream account state: fixed-size encoding with checked state-machine invariants.
//!
//! `unpack` re-validates every invariant on load, so a corrupted or hand-crafted account can
//! never be treated as valid on-chain state.

use pinocchio::{error::ProgramError, AccountView, Address};

use crate::{error::StreamError, pda};

pub const STREAM_DISCRIMINATOR: [u8; 8] = *b"NUSDCSTR";
pub const STREAM_VERSION: u8 = 1;
pub const STREAM_STATE_LEN: usize = 288;

const VERSION_OFFSET: usize = 8;
const STATUS_OFFSET: usize = 9;
const BUMP_OFFSET: usize = 10;
const RESERVED_OFFSET: usize = 11;
const STREAM_ID_OFFSET: usize = 16;
const SENDER_OFFSET: usize = 24;
const RECIPIENT_OFFSET: usize = 56;
const MINT_OFFSET: usize = 88;
const SOURCE_OFFSET: usize = 120;
const VAULT_OFFSET: usize = 152;
const RECIPIENT_TOKEN_OFFSET: usize = 184;
const TOTAL_OFFSET: usize = 216;
const CHUNK_OFFSET: usize = 224;
const SENT_OFFSET: usize = 232;
const EXECUTED_OFFSET: usize = 240;
const MAX_CHUNKS_OFFSET: usize = 248;
const CREATED_AT_OFFSET: usize = 256;
const START_OFFSET: usize = 264;
const NEXT_RELEASE_OFFSET: usize = 272;
const INTERVAL_OFFSET: usize = 280;

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StreamStatus {
    Uninitialized = 0,
    Active = 1,
    Completed = 2,
}

impl StreamStatus {
    pub const fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Uninitialized),
            1 => Some(Self::Active),
            2 => Some(Self::Completed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stream {
    pub version: u8,
    pub status: StreamStatus,
    pub stream_bump: u8,
    pub reserved: [u8; 5],
    pub stream_id: u64,
    pub sender: [u8; 32],
    pub recipient: [u8; 32],
    pub mint: [u8; 32],
    pub source_token_account: [u8; 32],
    pub vault_token_account: [u8; 32],
    pub recipient_token_account: [u8; 32],
    pub total_amount: u64,
    pub chunk_amount: u64,
    pub sent_amount: u64,
    pub executed_chunks: u64,
    pub max_chunks: u64,
    pub created_at: i64,
    pub start_timestamp: i64,
    pub next_release_timestamp: i64,
    pub interval_seconds: i64,
}

impl Stream {
    pub fn unpack(data: &[u8]) -> Result<Self, StreamError> {
        if data.len() != STREAM_STATE_LEN
            || data[..8] != STREAM_DISCRIMINATOR
            || data[VERSION_OFFSET] != STREAM_VERSION
        {
            return Err(StreamError::InvalidState);
        }
        let status =
            StreamStatus::from_u8(data[STATUS_OFFSET]).ok_or(StreamError::InvalidState)?;
        let reserved = read_array::<5>(data, RESERVED_OFFSET)?;
        if reserved != [0; 5] {
            return Err(StreamError::InvalidState);
        }
        let state = Self {
            version: data[VERSION_OFFSET],
            status,
            stream_bump: data[BUMP_OFFSET],
            reserved,
            stream_id: read_u64(data, STREAM_ID_OFFSET)?,
            sender: read_array(data, SENDER_OFFSET)?,
            recipient: read_array(data, RECIPIENT_OFFSET)?,
            mint: read_array(data, MINT_OFFSET)?,
            source_token_account: read_array(data, SOURCE_OFFSET)?,
            vault_token_account: read_array(data, VAULT_OFFSET)?,
            recipient_token_account: read_array(data, RECIPIENT_TOKEN_OFFSET)?,
            total_amount: read_u64(data, TOTAL_OFFSET)?,
            chunk_amount: read_u64(data, CHUNK_OFFSET)?,
            sent_amount: read_u64(data, SENT_OFFSET)?,
            executed_chunks: read_u64(data, EXECUTED_OFFSET)?,
            max_chunks: read_u64(data, MAX_CHUNKS_OFFSET)?,
            created_at: read_i64(data, CREATED_AT_OFFSET)?,
            start_timestamp: read_i64(data, START_OFFSET)?,
            next_release_timestamp: read_i64(data, NEXT_RELEASE_OFFSET)?,
            interval_seconds: read_i64(data, INTERVAL_OFFSET)?,
        };
        state.validate_invariants()?;
        Ok(state)
    }

    pub fn pack(&self, data: &mut [u8]) -> Result<(), StreamError> {
        if data.len() != STREAM_STATE_LEN {
            return Err(StreamError::InvalidState);
        }
        self.validate_invariants()?;
        data.fill(0);
        data[..8].copy_from_slice(&STREAM_DISCRIMINATOR);
        data[VERSION_OFFSET] = self.version;
        data[STATUS_OFFSET] = self.status as u8;
        data[BUMP_OFFSET] = self.stream_bump;
        data[RESERVED_OFFSET..RESERVED_OFFSET + 5].copy_from_slice(&self.reserved);
        write_u64(data, STREAM_ID_OFFSET, self.stream_id);
        write_array(data, SENDER_OFFSET, &self.sender);
        write_array(data, RECIPIENT_OFFSET, &self.recipient);
        write_array(data, MINT_OFFSET, &self.mint);
        write_array(data, SOURCE_OFFSET, &self.source_token_account);
        write_array(data, VAULT_OFFSET, &self.vault_token_account);
        write_array(data, RECIPIENT_TOKEN_OFFSET, &self.recipient_token_account);
        write_u64(data, TOTAL_OFFSET, self.total_amount);
        write_u64(data, CHUNK_OFFSET, self.chunk_amount);
        write_u64(data, SENT_OFFSET, self.sent_amount);
        write_u64(data, EXECUTED_OFFSET, self.executed_chunks);
        write_u64(data, MAX_CHUNKS_OFFSET, self.max_chunks);
        write_i64(data, CREATED_AT_OFFSET, self.created_at);
        write_i64(data, START_OFFSET, self.start_timestamp);
        write_i64(data, NEXT_RELEASE_OFFSET, self.next_release_timestamp);
        write_i64(data, INTERVAL_OFFSET, self.interval_seconds);
        Ok(())
    }

    /// Load and fully re-validate the stream state stored in `account`.
    pub fn load(account: &AccountView) -> Result<Self, ProgramError> {
        let data = account.try_borrow()?;
        Ok(Self::unpack(&data)?)
    }

    /// Re-validate and persist this stream state into `account`.
    pub fn store(&self, account: &AccountView) -> Result<(), ProgramError> {
        let mut data = account.try_borrow_mut()?;
        Ok(self.pack(&mut data)?)
    }

    /// Re-derive this stream's canonical PDA and confirm `stream_account` matches it exactly.
    pub fn validate_pda(
        &self,
        program_id: &Address,
        stream_account: &AccountView,
    ) -> Result<(), ProgramError> {
        if !stream_account.owned_by(program_id) {
            return Err(ProgramError::InvalidAccountOwner);
        }
        let expected = pda::stream_address(
            program_id,
            &Address::new_from_array(self.sender),
            &Address::new_from_array(self.recipient),
            &Address::new_from_array(self.mint),
            self.stream_id,
            self.stream_bump,
        )?;
        if stream_account.address() != &expected {
            return Err(StreamError::InvalidPda.into());
        }
        Ok(())
    }

    /// Amount not yet released.
    pub fn remaining(&self) -> Result<u64, StreamError> {
        self.total_amount
            .checked_sub(self.sent_amount)
            .ok_or(StreamError::ArithmeticOverflow)
    }

    /// The next chunk amount: `min(remaining, chunk_amount)`.
    pub fn next_amount(&self) -> Result<u64, StreamError> {
        Ok(core::cmp::min(self.remaining()?, self.chunk_amount))
    }

    fn validate_invariants(&self) -> Result<(), StreamError> {
        if self.version != STREAM_VERSION || self.reserved != [0; 5] {
            return Err(StreamError::InvalidState);
        }
        if self.total_amount == 0 || self.chunk_amount == 0 {
            return Err(StreamError::InvalidState);
        }
        let expected_max = ceil_div(self.total_amount, self.chunk_amount)
            .ok_or(StreamError::ArithmeticOverflow)?;
        if self.max_chunks != expected_max
            || self.sent_amount > self.total_amount
            || self.executed_chunks > self.max_chunks
        {
            return Err(StreamError::InvalidState);
        }
        let expected_sent = if self.executed_chunks == self.max_chunks {
            self.total_amount
        } else {
            self.executed_chunks
                .checked_mul(self.chunk_amount)
                .ok_or(StreamError::ArithmeticOverflow)?
        };
        if self.sent_amount != expected_sent {
            return Err(StreamError::InvalidState);
        }
        let status_ok = match self.status {
            StreamStatus::Uninitialized => false,
            StreamStatus::Active => {
                self.sent_amount < self.total_amount && self.executed_chunks < self.max_chunks
            }
            StreamStatus::Completed => {
                self.sent_amount == self.total_amount && self.executed_chunks == self.max_chunks
            }
        };
        if !status_ok {
            return Err(StreamError::InvalidState);
        }
        if self.created_at < 0
            || self.start_timestamp < self.created_at
            || self.interval_seconds < 0
        {
            return Err(StreamError::InvalidTimestamp);
        }
        let expected_next = next_release_timestamp(
            self.start_timestamp,
            self.executed_chunks,
            self.interval_seconds,
        )
        .ok_or(StreamError::ArithmeticOverflow)?;
        if self.next_release_timestamp != expected_next {
            return Err(StreamError::InvalidTimestamp);
        }
        Ok(())
    }
}

/// Overflow-safe ceiling division. Returns `None` for a zero divisor.
pub const fn ceil_div(value: u64, divisor: u64) -> Option<u64> {
    if divisor == 0 {
        return None;
    }
    let quotient = value / divisor;
    let remainder = value % divisor;
    quotient.checked_add(if remainder == 0 { 0 } else { 1 })
}

/// Derive schedule time from the original start, avoiding keeper-induced drift.
pub fn next_release_timestamp(start: i64, executed: u64, interval: i64) -> Option<i64> {
    if start < 0 || interval < 0 {
        return None;
    }
    let executed = i64::try_from(executed).ok()?;
    start.checked_add(executed.checked_mul(interval)?)
}

fn read_array<const N: usize>(data: &[u8], offset: usize) -> Result<[u8; N], StreamError> {
    data.get(
        offset
            ..offset
                .checked_add(N)
                .ok_or(StreamError::ArithmeticOverflow)?,
    )
    .ok_or(StreamError::InvalidState)?
    .try_into()
    .map_err(|_| StreamError::InvalidState)
}

fn read_u64(data: &[u8], offset: usize) -> Result<u64, StreamError> {
    Ok(u64::from_le_bytes(read_array(data, offset)?))
}

fn read_i64(data: &[u8], offset: usize) -> Result<i64, StreamError> {
    Ok(i64::from_le_bytes(read_array(data, offset)?))
}

fn write_array<const N: usize>(data: &mut [u8], offset: usize, value: &[u8; N]) {
    data[offset..offset + N].copy_from_slice(value);
}

fn write_u64(data: &mut [u8], offset: usize, value: u64) {
    write_array(data, offset, &value.to_le_bytes());
}

fn write_i64(data: &mut [u8], offset: usize, value: i64) {
    write_array(data, offset, &value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_stream(total: u64, chunk: u64) -> Stream {
        Stream {
            version: STREAM_VERSION,
            status: StreamStatus::Active,
            stream_bump: 250,
            reserved: [0; 5],
            stream_id: 9,
            sender: [1; 32],
            recipient: [2; 32],
            mint: [3; 32],
            source_token_account: [4; 32],
            vault_token_account: [5; 32],
            recipient_token_account: [6; 32],
            total_amount: total,
            chunk_amount: chunk,
            sent_amount: 0,
            executed_chunks: 0,
            max_chunks: ceil_div(total, chunk).unwrap_or(0),
            created_at: 100,
            start_timestamp: 100,
            next_release_timestamp: 100,
            interval_seconds: 3,
        }
    }

    #[test]
    fn state_round_trip_is_exact() {
        let state = active_stream(250, 100);
        let mut bytes = [0_u8; STREAM_STATE_LEN];
        assert_eq!(state.pack(&mut bytes), Ok(()));
        assert_eq!(Stream::unpack(&bytes), Ok(state));
    }

    #[test]
    fn ceil_div_and_partial_chunk_are_exact() {
        let state = active_stream(250, 100);
        assert_eq!(state.max_chunks, 3);
        assert_eq!(state.next_amount(), Ok(100));
        let mut near_final = state;
        near_final.sent_amount = 200;
        near_final.executed_chunks = 2;
        near_final.next_release_timestamp = 106;
        assert_eq!(near_final.next_amount(), Ok(50));
        assert_eq!(near_final.validate_invariants(), Ok(()));
    }

    #[test]
    fn timestamp_overflow_is_rejected() {
        assert_eq!(next_release_timestamp(i64::MAX, 1, 1), None);
        assert_eq!(next_release_timestamp(1, u64::MAX, 1), None);
        assert_eq!(next_release_timestamp(1, 1, -1), None);
    }

    #[test]
    fn bad_version_status_and_data_are_rejected() {
        let state = active_stream(100, 100);
        let mut bytes = [0_u8; STREAM_STATE_LEN];
        assert_eq!(state.pack(&mut bytes), Ok(()));
        bytes[VERSION_OFFSET] = 2;
        assert_eq!(Stream::unpack(&bytes), Err(StreamError::InvalidState));
        bytes[VERSION_OFFSET] = STREAM_VERSION;
        bytes[STATUS_OFFSET] = 99;
        assert_eq!(Stream::unpack(&bytes), Err(StreamError::InvalidState));
        assert_eq!(
            Stream::unpack(&bytes[..100]),
            Err(StreamError::InvalidState)
        );
    }
}
