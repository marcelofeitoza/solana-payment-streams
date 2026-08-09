//! Canonical PDA derivation shared by on-chain validation and off-chain clients.

use pinocchio::{error::ProgramError, Address};
use pinocchio_pubkey::pubkey;

use crate::{constants::STREAM_SEED, error::StreamError};

pub const ASSOCIATED_TOKEN_PROGRAM_ID: Address =
    Address::new_from_array(pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"));

/// Stream state PDA. Seeds: `["stream", sender, recipient, mint, stream_id (LE), bump]`.
pub fn stream_address(
    program_id: &Address,
    sender: &Address,
    recipient: &Address,
    mint: &Address,
    stream_id: u64,
    bump: u8,
) -> Result<Address, ProgramError> {
    let stream_id_bytes = stream_id.to_le_bytes();
    let bump_bytes = [bump];
    create_program_address(
        &[
            STREAM_SEED,
            sender.as_ref(),
            recipient.as_ref(),
            mint.as_ref(),
            &stream_id_bytes,
            &bump_bytes,
        ],
        program_id,
    )
}

/// Classic SPL associated-token-account address for `owner`/`mint`.
pub fn associated_token_address(owner: &Address, mint: &Address) -> Address {
    find_program_address(
        &[owner.as_ref(), pinocchio_token::ID.as_ref(), mint.as_ref()],
        &ASSOCIATED_TOKEN_PROGRAM_ID,
    )
    .0
}

fn create_program_address(
    seeds: &[&[u8]],
    program_id: &Address,
) -> Result<Address, ProgramError> {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        Address::create_program_address(seeds, program_id)
            .map_err(|_| StreamError::InvalidPda.into())
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        use solana_pubkey::Pubkey;
        let program = Pubkey::new_from_array(*program_id.as_array());
        let derived = Pubkey::create_program_address(seeds, &program)
            .map_err(|_| ProgramError::from(StreamError::InvalidPda))?;
        Ok(Address::new_from_array(derived.to_bytes()))
    }
}

fn find_program_address(seeds: &[&[u8]], program_id: &Address) -> (Address, u8) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    {
        Address::find_program_address(seeds, program_id)
    }
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    {
        use solana_pubkey::Pubkey;
        let program = Pubkey::new_from_array(*program_id.as_array());
        let (derived, bump) = Pubkey::find_program_address(seeds, &program);
        (Address::new_from_array(derived.to_bytes()), bump)
    }
}
