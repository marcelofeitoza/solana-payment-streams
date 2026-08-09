//! Initialize instruction
//!
//! Creates the stream PDA and moves the full `total_amount` from the sender's token account
//! into the stream's vault; only the stream PDA can move funds out of the vault thereafter.

use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_log::logger::Logger;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::TransferChecked;

use crate::{
    constants::{STREAM_SEED, USDC_DECIMALS},
    error::StreamError,
    instructions::{read_i64, read_u64},
    pda,
    state::{ceil_div, next_release_timestamp, Stream, StreamStatus, STREAM_STATE_LEN, STREAM_VERSION},
    token,
};

/// Accounts:
/// 0. `[signer, writable]` sender - funds the vault and pays for the new stream account
/// 1. `[]` recipient - stream beneficiary; only used to derive/validate its ATA
/// 2. `[]` mint - USDC-like SPL mint (6 decimals)
/// 3. `[writable]` source_token - sender's token account, debited for `total_amount`
/// 4. `[writable]` vault - stream's ATA; receives `total_amount`
/// 5. `[]` recipient_token - recipient's canonical ATA, bound into stream state
/// 6. `[writable]` stream_account - PDA created by this instruction
/// 7. `[]` token_program
/// 8. `[]` system_program
/// 9. `[]` clock_account
///
/// Data: `[stream_id: 8][total_amount: 8][chunk_amount: 8][start_timestamp: 8][interval_seconds: 8][stream_bump: 1]`
/// `start_timestamp == 0` means "use the current on-chain Clock timestamp".
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    if data.len() != 41 {
        return Err(StreamError::InvalidInstruction.into());
    }
    let [sender, recipient, mint, source_token, vault, recipient_token, stream_account, token_program, system_program, clock_account] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let stream_id = read_u64(data, 0)?;
    let total_amount = read_u64(data, 8)?;
    let chunk_amount = read_u64(data, 16)?;
    let start_timestamp = read_i64(data, 24)?;
    let interval_seconds = read_i64(data, 32)?;
    let stream_bump = data[40];

    if !sender.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    for writable in [sender, source_token, vault, stream_account] {
        if !writable.is_writable() {
            return Err(ProgramError::InvalidArgument);
        }
    }
    token::validate_programs(token_program, Some(system_program))?;
    if total_amount == 0 || chunk_amount == 0 || interval_seconds < 0 {
        return Err(StreamError::InvalidAmount.into());
    }
    if stream_account.lamports() != 0
        || stream_account.data_len() != 0
        || !stream_account.owned_by(&pinocchio_system::ID)
    {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let expected_stream = pda::stream_address(
        program_id,
        sender.address(),
        recipient.address(),
        mint.address(),
        stream_id,
        stream_bump,
    )?;
    if stream_account.address() != &expected_stream {
        return Err(StreamError::InvalidPda.into());
    }
    if vault.address() != &pda::associated_token_address(stream_account.address(), mint.address())
        || recipient_token.address()
            != &pda::associated_token_address(recipient.address(), mint.address())
    {
        return Err(StreamError::InvalidTokenAccount.into());
    }

    token::validate_mint(mint)?;
    let source_amount =
        token::validate_token_account(source_token, mint.address(), sender.address())?;
    let vault_amount =
        token::validate_token_account(vault, mint.address(), stream_account.address())?;
    token::validate_token_account(recipient_token, mint.address(), recipient.address())?;
    if source_amount < total_amount || vault_amount != 0 {
        return Err(StreamError::InvalidAmount.into());
    }

    let now = Clock::from_account_view(clock_account)?.unix_timestamp;
    if now < 0 {
        return Err(StreamError::InvalidTimestamp.into());
    }
    let start_timestamp = if start_timestamp == 0 {
        now
    } else if start_timestamp < now {
        return Err(StreamError::InvalidTimestamp.into());
    } else {
        start_timestamp
    };
    let max_chunks =
        ceil_div(total_amount, chunk_amount).ok_or(StreamError::ArithmeticOverflow)?;
    let stream_next_release = next_release_timestamp(start_timestamp, 0, interval_seconds)
        .ok_or(StreamError::ArithmeticOverflow)?;

    let rent_lamports = Rent::get()?.try_minimum_balance(STREAM_STATE_LEN)?;
    let stream_id_bytes = stream_id.to_le_bytes();
    let bump_bytes = [stream_bump];
    let signer_seeds = [
        Seed::from(STREAM_SEED),
        Seed::from(sender.address().as_ref()),
        Seed::from(recipient.address().as_ref()),
        Seed::from(mint.address().as_ref()),
        Seed::from(&stream_id_bytes),
        Seed::from(&bump_bytes),
    ];
    CreateAccount {
        from: sender,
        to: stream_account,
        lamports: rent_lamports,
        space: STREAM_STATE_LEN as u64,
        owner: program_id,
    }
    .invoke_signed(&[Signer::from(&signer_seeds)])?;

    let stream = Stream {
        version: STREAM_VERSION,
        status: StreamStatus::Active,
        stream_bump,
        reserved: [0; 5],
        stream_id,
        sender: *sender.address().as_array(),
        recipient: *recipient.address().as_array(),
        mint: *mint.address().as_array(),
        source_token_account: *source_token.address().as_array(),
        vault_token_account: *vault.address().as_array(),
        recipient_token_account: *recipient_token.address().as_array(),
        total_amount,
        chunk_amount,
        sent_amount: 0,
        executed_chunks: 0,
        max_chunks,
        created_at: now,
        start_timestamp,
        next_release_timestamp: stream_next_release,
        interval_seconds,
    };
    stream.store(stream_account)?;

    TransferChecked {
        from: source_token,
        mint,
        to: vault,
        authority: sender,
        amount: total_amount,
        decimals: USDC_DECIMALS,
    }
    .invoke()?;

    let mut logger = Logger::<160>::default();
    logger
        .append("initialize stream_id=")
        .append(stream_id)
        .append(" total=")
        .append(total_amount)
        .append(" chunk=")
        .append(chunk_amount)
        .append(" max_chunks=")
        .append(max_chunks)
        .log();
    Ok(())
}
