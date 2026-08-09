//! Initialize instruction (base layer, sender-invoked)
//!
//! Creates the stream PDA and moves the full `total_amount` from the sender's token account
//! into a non-delegated SPL escrow, so it survives delegation to the Ephemeral Rollup.

use pinocchio::{
    cpi::Signer,
    error::ProgramError,
    sysvars::{rent::Rent, Sysvar},
    AccountView, Address, ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;
use pinocchio_token::instructions::TransferChecked;

use crate::{
    constants::{DEFAULT_FEE_RESERVE_LAMPORTS, USDC_DECIMALS},
    error::StreamError,
    instructions::read_u64,
    pda,
    state::{StreamState, STREAM_STATE_LEN},
    token,
};

/// Accounts:
/// 0. `[signer, writable]` sender - funds the escrow and pays for the new stream account
/// 1. `[]` recipient - stream beneficiary
/// 2. `[]` mint - USDC-like SPL mint (6 decimals)
/// 3. `[]` source_token - sender's token account, debited for `total_amount`
/// 4. `[]` destination_token - recipient's token account, bound into stream state
/// 5. `[writable]` escrow_token - non-delegated escrow ATA; receives `total_amount`
/// 6. `[]` escrow_authority - PDA authority over `escrow_token`
/// 7. `[writable]` stream - PDA created by this instruction
/// 8. `[]` token_program
/// 9. `[]` system_program
/// 10. `[]` validator - local Ephemeral Rollup validator this stream will delegate to
///
/// Data: `[total_amount: 8][chunk_amount: 8][interval_ms: 8][fee_reserve_lamports: 8][bump: 1][escrow_bump: 1]`
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    if data.len() != 34 {
        return Err(StreamError::InvalidInstruction.into());
    }
    let [sender, recipient, mint, source_token, destination_token, escrow_token, escrow_authority, stream, token_program, system_program, validator] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let total_amount = read_u64(data, 0)?;
    let chunk_amount = read_u64(data, 8)?;
    let interval_ms = read_u64(data, 16)?;
    let fee_reserve_lamports = read_u64(data, 24)?;
    let bump = data[32];
    let escrow_bump = data[33];

    if !sender.is_signer() || !sender.is_writable() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if token_program.address() != &pinocchio_token::ID
        || system_program.address() != &pinocchio_system::ID
        || !token_program.executable()
        || !system_program.executable()
    {
        return Err(ProgramError::IncorrectProgramId);
    }
    if total_amount == 0
        || chunk_amount == 0
        || chunk_amount > total_amount
        || interval_ms == 0
        || interval_ms > i64::MAX as u64
        || fee_reserve_lamports < DEFAULT_FEE_RESERVE_LAMPORTS
    {
        return Err(StreamError::InvalidAmount.into());
    }
    if stream.lamports() != 0 || stream.data_len() != 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    let expected_stream = pda::stream_address_from_parts(
        program_id,
        sender.address(),
        recipient.address(),
        mint.address(),
        bump,
    )?;
    if stream.address() != &expected_stream {
        return Err(StreamError::InvalidPda.into());
    }
    let expected_escrow = pda::escrow_authority_address(program_id, stream.address(), escrow_bump)?;
    if escrow_authority.address() != &expected_escrow {
        return Err(StreamError::InvalidPda.into());
    }

    token::validate_mint(mint)?;
    let source_amount = token::validate_token_account(source_token, mint.address(), sender.address())?;
    token::validate_token_account(destination_token, mint.address(), recipient.address())?;
    let escrow_amount =
        token::validate_token_account(escrow_token, mint.address(), escrow_authority.address())?;
    if source_amount < total_amount || escrow_amount != 0 {
        return Err(StreamError::InvalidAmount.into());
    }

    let rent_lamports = Rent::get()?.try_minimum_balance(STREAM_STATE_LEN)?;
    let state_lamports = rent_lamports
        .checked_add(fee_reserve_lamports)
        .ok_or(StreamError::ArithmeticOverflow)?;
    let state = StreamState {
        sender: *sender.address().as_array(),
        recipient: *recipient.address().as_array(),
        mint: *mint.address().as_array(),
        source_token_account: *source_token.address().as_array(),
        destination_token_account: *destination_token.address().as_array(),
        escrow_token_account: *escrow_token.address().as_array(),
        total_amount,
        chunk_amount,
        sent_amount: 0,
        validator: *validator.address().as_array(),
        interval_ms,
        active: true,
        bump,
        escrow_bump,
        scheduled: false,
        task_id: crate::constants::CRANK_TASK_ID,
    };
    let signer_seeds = pda::stream_signer_seeds(&state);
    CreateAccount {
        from: sender,
        to: stream,
        lamports: state_lamports,
        space: STREAM_STATE_LEN as u64,
        owner: program_id,
    }
    .invoke_signed(&[Signer::from(&signer_seeds)])?;

    state.store(stream)?;

    TransferChecked {
        from: source_token,
        mint,
        to: escrow_token,
        authority: sender,
        amount: total_amount,
        decimals: USDC_DECIMALS,
    }
    .invoke()
}
