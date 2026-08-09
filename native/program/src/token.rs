//! Classic SPL Token validation and PDA-signed CPI helpers shared by every instruction.

use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address, ProgramResult,
};
use pinocchio_token::{
    instructions::TransferChecked,
    state::{Mint, TokenAccount},
};

use crate::{
    constants::{STREAM_SEED, USDC_DECIMALS},
    error::StreamError,
    pda,
    state::Stream,
};

/// Confirm `token_program` (and optionally `system_program`) are the real, executable programs.
pub fn validate_programs(
    token_program: &AccountView,
    system_program: Option<&AccountView>,
) -> ProgramResult {
    if token_program.address() != &pinocchio_token::ID || !token_program.executable() {
        return Err(ProgramError::IncorrectProgramId);
    }
    if let Some(system_program) = system_program {
        if system_program.address() != &pinocchio_system::ID || !system_program.executable() {
            return Err(ProgramError::IncorrectProgramId);
        }
    }
    Ok(())
}

pub fn validate_mint(mint: &AccountView) -> ProgramResult {
    let state = Mint::from_account_view(mint).map_err(|_| StreamError::InvalidMint)?;
    if !state.is_initialized() || state.decimals() != USDC_DECIMALS {
        return Err(StreamError::InvalidMint.into());
    }
    Ok(())
}

pub fn validate_token_account(
    account: &AccountView,
    mint: &Address,
    owner: &Address,
) -> Result<u64, ProgramError> {
    let state = TokenAccount::from_account_view(account)
        .map_err(|_| ProgramError::from(StreamError::InvalidTokenAccount))?;
    if !state.is_initialized() || state.mint() != mint || state.owner() != owner {
        return Err(StreamError::InvalidTokenAccount.into());
    }
    Ok(state.amount())
}

/// Validate `mint`/`vault`/`recipient_token` against the stream's recorded accounts and their
/// canonical associated-token addresses. Returns the vault's current balance.
pub fn validate_bound_accounts(
    stream_account: &AccountView,
    stream: &Stream,
    vault: &AccountView,
    recipient_token: &AccountView,
    mint: &AccountView,
) -> Result<u64, ProgramError> {
    if mint.address() != &Address::new_from_array(stream.mint) {
        return Err(StreamError::InvalidMint.into());
    }
    validate_mint(mint)?;
    let expected_vault = pda::associated_token_address(stream_account.address(), mint.address());
    if vault.address() != &Address::new_from_array(stream.vault_token_account)
        || vault.address() != &expected_vault
    {
        return Err(StreamError::InvalidTokenAccount.into());
    }
    let vault_amount = validate_token_account(vault, mint.address(), stream_account.address())?;

    let recipient = Address::new_from_array(stream.recipient);
    let expected_recipient = pda::associated_token_address(&recipient, mint.address());
    if recipient_token.address() != &Address::new_from_array(stream.recipient_token_account)
        || recipient_token.address() != &expected_recipient
    {
        return Err(StreamError::InvalidTokenAccount.into());
    }
    validate_token_account(recipient_token, mint.address(), &recipient)?;

    Ok(vault_amount)
}

/// Move `amount` out of the vault, signed by the stream PDA.
pub fn transfer_from_vault(
    stream_account: &AccountView,
    vault: &AccountView,
    mint: &AccountView,
    destination: &AccountView,
    stream: &Stream,
    amount: u64,
) -> ProgramResult {
    let stream_id_bytes = stream.stream_id.to_le_bytes();
    let bump_bytes = [stream.stream_bump];
    let seeds = [
        Seed::from(STREAM_SEED),
        Seed::from(stream.sender.as_ref()),
        Seed::from(stream.recipient.as_ref()),
        Seed::from(stream.mint.as_ref()),
        Seed::from(&stream_id_bytes),
        Seed::from(&bump_bytes),
    ];
    TransferChecked {
        from: vault,
        mint,
        to: destination,
        authority: stream_account,
        amount,
        decimals: USDC_DECIMALS,
    }
    .invoke_signed(&[Signer::from(&seeds)])
}
