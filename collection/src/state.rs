use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::pubkey::Pubkey;

// /// Аккаунт всей коллекции (PDA: ["collection", authority])
// #[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
// pub struct CollectionState {
//     pub authority: Pubkey,        // кто управляет коллекцией
//     pub total_supply: u64,        // максимум NFT
//     pub minted_count: u64,        // уже заминтировано
//     pub price_lamports: u64,      // цена минта в lamports
//     pub is_paused: bool,          // кастомная пауза минтинга
//     pub whitelist_enabled: bool,  // включить вайтлист
//     pub uri_base: String,         // базовый URI метаданных
// }

/// Аккаунт коллекции
/// PDA: ["collection", authority]
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct CollectionState {
    pub authority: Pubkey,
    pub total_supply: u64,
    pub minted_count: u64,
    pub uri_base: String,

    /// Награда за каждое клонирование (из treasury)
    pub clone_reward_lamports: u64,

    /// Стоимость авто-минта для AddressV6
    pub auto_mint_price_lamports: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq)]
pub enum NftKind {
    Default,
    SBT,
    Address,           // proxy → произвольный target, fee → collection authority
    AddressV2,         // proxy → произвольный target, без fee
    AddressV3,         // статическое хранилище SOL, вывод вручную
    AddressV3j,        // статическое хранилище SOL + SPL токены
    AddressV4Fraction, // дробное: владелец задаёт сколько % уходит дочерним NFT
    AddressV5,         // proxy → произвольный target, fee → владельцу NFT
    AddressV6,         // авто-минт при входящем платеже
    Clone,             // вирусная копия v1 — оригинал остаётся, копия идёт получателю
    CloneV2,           // вирусная копия v2 — двухуровневые награды
}

/// Одна дочерняя доля для V4Fraction
/// child_nft_pda — PDA дочернего NFT которому идёт share_bps процентов
#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct FractionChild {
    pub child_nft_pda: Pubkey, // PDA дочернего NFT
    pub share_bps: u16,        // сколько % входящего суммы туда идёт (10000 = 100%)
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone)]
pub struct NftState {
    pub kind: NftKind,
    pub collection: Pubkey,
    pub mint: Pubkey,

    /// Текущий владелец NFT
    pub owner: Pubkey,

    pub mint_index: u64,
    pub is_burned: bool,

    // ── Прокси (Address, AddressV2, AddressV5, AddressV6) ──────────────
    /// Куда проксируются средства (задаётся при минте)
    pub proxy_target: Pubkey,

    /// Комиссия в bps (500 = 5%). 0 = нет.
    pub proxy_fee_bps: u16,

    /// Куда идёт fee:
    /// - Address → collection authority
    /// - AddressV5 → owner NFT
    /// - AddressV6 → owner NFT
    pub fee_recipient: Pubkey,

    // ── V4Fraction ──────────────────────────────────────────────────────
    /// Может ли этот NFT быть раздроблен (false для дочерних)
    pub can_fraction: bool,

    /// Список дочерних NFT и их доли. Остаток (10000 - сумма) → proxy_target
    pub fraction_children: Vec<FractionChild>,

    // ── Clone / CloneV2 ─────────────────────────────────────────────────
    /// PDA оригинального NFT (None если это оригинал)
    pub original_nft: Option<Pubkey>,

    /// PDA родителя поколения-1 (только для CloneV2 поколения >= 2)
    pub parent_nft: Option<Pubkey>,

    /// Поколение: 0 = оригинал, 1 = первая волна, 2+ = дальше
    pub generation: u8,

    /// Сколько раз скопирован
    pub clone_count: u64,

    /// Накопленные награды за клонирование (в lamports)
    pub pending_rewards_lamports: u64,

    // ── Статистика ──────────────────────────────────────────────────────
    pub total_received_lamports: u64,
    pub total_forwarded_lamports: u64,

    // Новые поля для метаданных
    name: String,
    symbol: String,
    uri: String,
    seller_fee_bps: u16,
}