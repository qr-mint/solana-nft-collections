use borsh::{BorshDeserialize, BorshSerialize};

#[derive(BorshSerialize, BorshDeserialize, Debug)]
pub enum NftInstruction {
    /// Создать коллекцию
    /// Accounts: [authority(signer), collection_pda(writable), system_program]
    InitializeCollection {
        total_supply: u64,
        price_lamports: u64,
        whitelist_enabled: bool,
        uri_base: String,
    },

    /// Минтить NFT
    /// Accounts: [payer(signer), collection_pda(writable), nft_pda(writable),
    ///            mint(writable), token_account(writable), metadata_account(writable),
    ///            spl_token_program, metadata_program, system_program, rent]
    MintNft,

    /// Кастомная логика: прокачать NFT (набрать опыт)
    /// Accounts: [owner(signer), nft_pda(writable), collection_pda]
    GainExperience { amount: u64 },

    /// Перевод с кастомной проверкой
    /// Accounts: [owner(signer), nft_pda(writable), new_owner]
    TransferNft { new_owner: Pubkey },

    /// Управление: поставить минт на паузу
    /// Accounts: [authority(signer), collection_pda(writable)]
    SetPaused { paused: bool },
}