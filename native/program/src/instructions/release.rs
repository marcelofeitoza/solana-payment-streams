//! Release instruction
//!
//! Permissionless: any signer may submit it, but only the stream PDA can move funds, and only
//! into the recipient's bound token account. A stale or already-applied `expected_chunk_index`
//! is a successful no-op, so a raced keeper can never double-spend a chunk or revert on replay.

use pinocchio::{error::ProgramError, sysvars::clock::Clock, AccountView, Address, ProgramResult};
use pinocchio_log::logger::Logger;

use crate::{error::StreamError, instructions::read_u64, state::StreamStatus, token};

/// Accounts:
/// 0. `[signer]` caller - permissionless fee payer; not otherwise authorized
/// 1. `[writable]` stream_account
/// 2. `[writable]` vault - stream's ATA, debited for the chunk
/// 3. `[writable]` recipient_token - recipient's bound ATA, credited for the chunk
/// 4. `[]` mint
/// 5. `[]` token_program
/// 6. `[]` clock_account
///
/// Data: `[expected_chunk_index: 8]`
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    if data.len() != 8 {
        return Err(StreamError::InvalidInstruction.into());
    }
    let [caller, stream_account, vault, recipient_token, mint, token_program, clock_account] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let expected_chunk_index = read_u64(data, 0)?;

    if !caller.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    for writable in [stream_account, vault, recipient_token] {
        if !writable.is_writable() {
            return Err(ProgramError::InvalidArgument);
        }
    }
    token::validate_programs(token_program, None)?;

    let mut stream = crate::state::Stream::load(stream_account)?;
    stream.validate_pda(program_id, stream_account)?;
    let vault_amount =
        token::validate_bound_accounts(stream_account, &stream, vault, recipient_token, mint)?;

    if expected_chunk_index > stream.executed_chunks {
        return Err(StreamError::FutureChunkIndex.into());
    }
    if expected_chunk_index < stream.executed_chunks || stream.status == StreamStatus::Completed {
        log_noop(stream.stream_id, expected_chunk_index, stream.executed_chunks);
        return Ok(());
    }
    if stream.status != StreamStatus::Active {
        return Err(StreamError::InvalidStatus.into());
    }

    let now = Clock::from_account_view(clock_account)?.unix_timestamp;
    if now < stream.next_release_timestamp {
        return Err(StreamError::ReleaseNotDue.into());
    }
    let remaining = stream.remaining()?;
    let amount = stream.next_amount()?;
    if amount == 0 || vault_amount != remaining || vault_amount < amount {
        return Err(StreamError::InvalidAmount.into());
    }

    let new_sent = stream
        .sent_amount
        .checked_add(amount)
        .ok_or(StreamError::ArithmeticOverflow)?;
    let new_executed = stream
        .executed_chunks
        .checked_add(1)
        .ok_or(StreamError::ArithmeticOverflow)?;
    let new_next = crate::state::next_release_timestamp(
        stream.start_timestamp,
        new_executed,
        stream.interval_seconds,
    )
    .ok_or(StreamError::ArithmeticOverflow)?;
    if new_sent > stream.total_amount || new_executed > stream.max_chunks {
        return Err(StreamError::InvalidAmount.into());
    }

    token::transfer_from_vault(stream_account, vault, mint, recipient_token, &stream, amount)?;

    stream.sent_amount = new_sent;
    stream.executed_chunks = new_executed;
    stream.next_release_timestamp = new_next;
    if new_sent == stream.total_amount {
        stream.status = StreamStatus::Completed;
    }
    stream.store(stream_account)?;

    let mut logger = Logger::<192>::default();
    logger
        .append("release stream_id=")
        .append(stream.stream_id)
        .append(" index=")
        .append(expected_chunk_index)
        .append(" amount=")
        .append(amount)
        .append(" sent=")
        .append(new_sent)
        .append(" status=")
        .append(stream.status as u8)
        .log();
    Ok(())
}

fn log_noop(stream_id: u64, expected: u64, executed: u64) {
    let mut logger = Logger::<128>::default();
    logger
        .append("release_noop stream_id=")
        .append(stream_id)
        .append(" expected=")
        .append(expected)
        .append(" executed=")
        .append(executed)
        .log();
}
