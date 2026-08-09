//! Classic SPL Token validation and escrow-authority-signed CPI helpers.

use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    AccountView, Address, ProgramResult,
};
use pinocchio_token::{
    instructions::{CloseAccount, TransferChecked},
    state::{Mint, TokenAccount},
};

use crate::{
    constants::{ESCROW_AUTHORITY_SEED, USDC_DECIMALS},
    error::StreamError,
};

pub fn validate_mint(mint: &AccountView) -> Result<(), ProgramError> {
    let mint_state = Mint::from_account_view(mint)?;
    if !mint_state.is_initialized() || mint_state.decimals() != USDC_DECIMALS {
        return Err(StreamError::InvalidTokenAccount.into());
    }
    Ok(())
}

pub fn validate_token_account(
    account: &AccountView,
    mint: &Address,
    owner: &Address,
) -> Result<u64, ProgramError> {
    let token = TokenAccount::from_account_view(account)?;
    if !token.is_initialized() || token.mint() != mint || token.owner() != owner {
        return Err(StreamError::InvalidTokenAccount.into());
    }
    Ok(token.amount())
}

/// Move `amount` out of the escrow, signed by the escrow-authority PDA.
pub fn transfer_from_escrow(
    stream: &AccountView,
    escrow_token: &AccountView,
    mint: &AccountView,
    destination_token: &AccountView,
    escrow_authority: &AccountView,
    escrow_bump: u8,
    amount: u64,
) -> ProgramResult {
    let seeds = escrow_signer_seeds(stream.address(), &escrow_bump);
    TransferChecked {
        from: escrow_token,
        mint,
        to: destination_token,
        authority: escrow_authority,
        amount,
        decimals: USDC_DECIMALS,
    }
    .invoke_signed(&[Signer::from(&seeds)])
}

/// Close the (empty) escrow token account, signed by the escrow-authority PDA.
pub fn close_escrow(
    stream: &AccountView,
    escrow_token: &AccountView,
    destination: &AccountView,
    escrow_authority: &AccountView,
    escrow_bump: u8,
) -> ProgramResult {
    let seeds = escrow_signer_seeds(stream.address(), &escrow_bump);
    CloseAccount {
        account: escrow_token,
        destination,
        authority: escrow_authority,
    }
    .invoke_signed(&[Signer::from(&seeds)])
}

fn escrow_signer_seeds<'a>(stream: &'a Address, escrow_bump: &'a u8) -> [Seed<'a>; 3] {
    [
        Seed::from(ESCROW_AUTHORITY_SEED),
        Seed::from(stream.as_ref()),
        Seed::from(core::slice::from_ref(escrow_bump)),
    ]
}
