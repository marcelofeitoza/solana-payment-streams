//! SettleChunk instruction (base layer, DLP Magic Action callback)
//!
//! Executed by the DLP while the stream is still delegated. Moves exactly one chunk from the
//! non-delegated escrow to the recipient. The escrow's exact balance is the replay guard: an
//! immediate redelivery of the same action is a successful no-op, never a double transfer.

use ephemeral_rollups_pinocchio::consts::DELEGATION_PROGRAM_ID;
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

use crate::{
    error::StreamError,
    instructions::read_u64,
    state::StreamState,
    token,
    validation::validate_action_suffix,
};

/// Accounts:
/// 0. `[]` stream - delegated (owned by the Delegation Program) at settlement time
/// 1. `[]` mint
/// 2. `[writable]` escrow_token
/// 3. `[writable]` destination_token - recipient's bound token account
/// 4. `[]` escrow_authority
/// 5. `[]` token_program
/// 6. `[]` source_program - this program, appended by the DLP's secure CallHandler suffix
/// 7. `[]` action_escrow_authority - DLP-appended suffix account
/// 8. `[signer]` action_escrow_signer - DLP-appended suffix account
///
/// Data: `[amount: 8][expected_sent: 8]`
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    if data.len() != 16 {
        return Err(StreamError::InvalidInstruction.into());
    }
    let [stream, mint, escrow_token, destination_token, escrow_authority, token_program, source_program, action_escrow_authority, action_escrow_signer] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let amount = read_u64(data, 0)?;
    let expected_sent = read_u64(data, 8)?;

    if !stream.owned_by(&DELEGATION_PROGRAM_ID) || token_program.address() != &pinocchio_token::ID
    {
        return Err(ProgramError::InvalidAccountOwner);
    }

    let state = StreamState::load(stream)?;
    state.validate_pdas(program_id, stream, escrow_authority)?;
    validate_action_suffix(
        program_id,
        stream.address(),
        source_program,
        action_escrow_authority,
        action_escrow_signer,
    )?;
    if !state.active
        || expected_sent != state.sent_amount
        || expected_sent >= state.total_amount
        || amount != state.chunk_amount
        || expected_sent < amount
        || expected_sent % state.chunk_amount != 0
    {
        return Err(StreamError::InvalidAmount.into());
    }
    if mint.address() != &Address::new_from_array(state.mint)
        || escrow_token.address() != &Address::new_from_array(state.escrow_token_account)
        || destination_token.address() != &Address::new_from_array(state.destination_token_account)
    {
        return Err(StreamError::InvalidTokenAccount.into());
    }

    token::validate_mint(mint)?;
    let escrow_balance =
        token::validate_token_account(escrow_token, mint.address(), escrow_authority.address())?;
    token::validate_token_account(
        destination_token,
        mint.address(),
        &Address::new_from_array(state.recipient),
    )?;
    let previous_sent = expected_sent
        .checked_sub(amount)
        .ok_or(StreamError::ArithmeticOverflow)?;
    let expected_escrow_balance = state
        .total_amount
        .checked_sub(previous_sent)
        .ok_or(StreamError::ArithmeticOverflow)?;
    let settled_escrow_balance = state
        .total_amount
        .checked_sub(expected_sent)
        .ok_or(StreamError::ArithmeticOverflow)?;
    // The token escrow is intentionally not delegated, so its exact balance is the base
    // layer's replay guard. An immediate redelivery is a successful no-op; the same action
    // cannot transfer twice even with a new signature.
    if escrow_balance == settled_escrow_balance {
        return Ok(());
    }
    if escrow_balance != expected_escrow_balance {
        return Err(StreamError::InvalidAmount.into());
    }

    token::transfer_from_escrow(
        stream,
        escrow_token,
        mint,
        destination_token,
        escrow_authority,
        state.escrow_bump,
        amount,
    )
}
