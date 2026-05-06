use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

use crate::state::NftKind;

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum NftInstruction {
    /// Initialize collection.
    /// Accounts: [authority(signer), collection_pda(writable), system_program]
    InitializeCollection {
        total_supply: u64,
        uri_base: String,
        clone_reward_lamports: u64,
        auto_mint_price_lamports: u64,
    },

    /// Mint a new NFT state account.
    /// Accounts: [payer(signer), collection_pda(writable), nft_pda(writable), mint, system_program]
    MintNft {
        kind: NftKind,
        proxy_target: Pubkey,
        proxy_fee_bps: u16,
        fraction_children: Vec<(Pubkey, u16)>,
    },

    /// Forward lamports from NFT PDA to proxy target / fractions.
    /// Accounts: [caller, nft_pda(writable), proxy_target(writable), fee_recipient(writable), system_program, ...children]
    ProxyForward,

    /// Withdraw lamports from V3/V3j storage.
    /// Accounts: [owner(signer), nft_pda(writable), destination(writable), system_program]
    Withdraw { amount_lamports: u64 },

    /// Withdraw SPL tokens from V3j storage.
    /// Accounts: [owner(signer), nft_pda, src_token, dst_token, token_program]
    WithdrawTokens { amount: u64 },

    /// Claim pending clone rewards from collection treasury.
    /// Accounts: [owner(signer), nft_pda(writable), collection_pda(writable), wallet(writable)]
    ClaimCloneRewards,

    /// Update fraction children for V4Fraction.
    /// Accounts: [owner(signer), nft_pda(writable)]
    UpdateFractions { fraction_children: Vec<(Pubkey, u16)> },

    /// Auto-mint triggered by AddressV6 payment.
    /// Accounts: [sender(signer), collection_pda(writable), trigger_pda, new_nft_pda(writable), new_mint, fee_recipient(writable), system_program]
    AutoMint {
        new_nft_kind: NftKind,
        new_proxy_target: Pubkey,
    },
    
    /// Clone Clone/CloneV2 NFT.
    /// Accounts: [owner(signer), collection_pda(writable), original_pda(writable), new_nft_pda(writable), new_mint, system_program, (optional gen0_pda)]
    Transfer {
        new_owner: Pubkey,
    }

    BurnNft,
}