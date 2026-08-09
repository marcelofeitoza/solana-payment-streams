//! Finalize instruction (base layer, DLP Magic Action callback)
//!
//! Executed by the DLP after the stream has been committed and undelegated back to this
//! program on natural completion. Pays the final chunk to the recipient, closes the escrow,
//! and closes the stream account, refunding its rent to the sender.

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

use crate::{
    error::StreamError,
    instructions::read_u64,
    state::StreamState,
    token,
    validation::validate_action_suffix,
};

/// Accounts:
/// 0. `[writable]` stream - closed; rent goes to sender
/// 1. `[]` mint
/// 2. `[writable]` escrow_token - closed to sender after the payout
/// 3. `[writable]` destination_token - recipient's bound ATA
/// 4. `[]` escrow_authority
/// 5. `[writable]` sender - must equal the stream's recorded sender
/// 6. `[]` token_program
/// 7. `[]` source_program - this program, appended by the DLP's secure CallHandler suffix
/// 8. `[]` action_escrow_authority - DLP-appended suffix account
/// 9. `[signer]` action_escrow_signer - DLP-appended suffix account
///
/// Data: `[amount: 8][expected_sent: 8]`
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
    if data.len() != 16 {
        return Err(StreamError::InvalidInstruction.into());
    }
    let [stream, mint, escrow_token, destination_token, escrow_authority, sender, token_program, source_program, action_escrow_authority, action_escrow_signer] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };
    let amount = read_u64(data, 0)?;
    let expected_sent = read_u64(data, 8)?;

    if !stream.owned_by(program_id) || token_program.address() != &pinocchio_token::ID {
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
    if state.active || sender.address() != &Address::new_from_array(state.sender) {
        return Err(StreamError::InvalidState.into());
    }

    let remainder = state.total_amount % state.chunk_amount;
    let final_chunk = if remainder == 0 {
        state.chunk_amount
    } else {
        remainder
    };
    if expected_sent != state.total_amount
        || state.sent_amount != state.total_amount
        || amount != final_chunk
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
    if escrow_balance != amount {
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
    )?;
    token::close_escrow(stream, escrow_token, sender, escrow_authority, state.escrow_bump)?;

    let refunded_lamports = sender
        .lamports()
        .checked_add(stream.lamports())
        .ok_or(StreamError::ArithmeticOverflow)?;
    sender.set_lamports(refunded_lamports);
    stream.set_lamports(0);
    stream.close()
}
