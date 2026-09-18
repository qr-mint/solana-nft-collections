use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};
use spl_token::state::Account as TokenAccount;

use crate::{
    error::NftError,
    instruction::NftInstruction,
    state::{CollectionState, FractionChild, NftKind, NftState},
};

use mpl_token_metadata::{
    instructions::CreateMetadataAccountV3Builder,
    instructions::UpdateV1Builder,
    types::DataV2,
    types::Data,
    ID as METADATA_PROGRAM_ID,
};
use spl_associated_token_account::instruction::create_associated_token_account;
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    match NftInstruction::try_from_slice(data)
        .map_err(|_| ProgramError::InvalidInstructionData)?
    {
        NftInstruction::InitializeCollection {
            collection_id, total_supply, uri_base,
            clone_reward_lamports, auto_mint_price_lamports,
        } => process_init_collection(
            program_id, accounts, collection_id,
            total_supply, uri_base,
            clone_reward_lamports, auto_mint_price_lamports,
        ),
       
        NftInstruction::MintNft { name, symbol, uri, seller_fee_bps, kind, proxy_target, proxy_fee_bps, fraction_children } =>
            process_mint_nft(program_id, accounts, name, symbol, uri, seller_fee_bps, kind, proxy_target, proxy_fee_bps, fraction_children),

        NftInstruction::ProxyForward =>
            process_proxy_forward(program_id, accounts),

        NftInstruction::Withdraw =>
            process_withdraw(program_id, accounts),

        NftInstruction::WithdrawTokens =>
            process_withdraw_tokens(program_id, accounts),

        NftInstruction::ClaimCloneRewards =>
            process_claim_clone_rewards(program_id, accounts),

        NftInstruction::UpdateFractions { fraction_children } =>
            process_update_fractions(program_id, accounts, fraction_children),

        NftInstruction::AutoMint { new_nft_kind, new_proxy_target } =>
            process_auto_mint(program_id, accounts, new_nft_kind, new_proxy_target),

        NftInstruction::Transfer { new_owner } =>
            process_transfer(program_id, accounts, new_owner),

        NftInstruction::BurnNft =>
            process_burn_nft(program_id, accounts),
        
        NftInstruction::UpdateNft { name, symbol, uri, seller_fee_bps, proxy_target, proxy_fee_bps } =>
            process_update_nft(program_id, accounts, name, symbol, uri, seller_fee_bps, proxy_target, proxy_fee_bps),
        
        NftInstruction::TransferCollection { new_authority } =>
            process_transfer_collection(program_id, accounts, new_authority),
            
    }
}

// ════════════════════════════════════════════════════════
// INIT COLLECTION
// ════════════════════════════════════════════════════════

fn process_init_collection(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    collection_id: u64,
    total_supply: u64,
    uri_base: String,
    clone_reward_lamports: u64,
    auto_mint_price_lamports: u64,
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let authority   = next_account_info(iter)?;
    let coll_pda    = next_account_info(iter)?;
    let system_prog = next_account_info(iter)?;

    require_signer(authority)?;
    require_program(&solana_program::system_program::id(), system_prog.key)?;

    let (pda, bump) = Pubkey::find_program_address(
        &[b"collection", authority.key.as_ref(), &collection_id.to_le_bytes()],
        program_id,
    );
    require_key(&pda, coll_pda.key, NftError::InvalidPda)?;

    let state = CollectionState {
        authority: *authority.key,
        collection_id,
        total_supply,
        minted_count: 0,
        uri_base,
        clone_reward_lamports,
        auto_mint_price_lamports,
    };

    create_pda_account(
        program_id, authority, coll_pda, system_prog,
        &state.try_to_vec()?,
        &[b"collection", authority.key.as_ref(), &collection_id.to_le_bytes(), &[bump]],
    )
}

// ════════════════════════════════════════════════════════
// MINT NFT
// ════════════════════════════════════════════════════════

fn process_mint_nft(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    name: String,
    symbol: String,
    uri: String,
    seller_fee_bps: u16,
    kind: NftKind,
    proxy_target: Pubkey,
    proxy_fee_bps: u16,
    fraction_children_raw: Vec<(Pubkey, u16)>,
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let payer            = next_account_info(iter)?;
    let coll_pda         = next_account_info(iter)?;
    let nft_pda          = next_account_info(iter)?;
    let mint             = next_account_info(iter)?;
    let token_account    = next_account_info(iter)?;
    let owner            = next_account_info(iter)?;
    let metadata_account = next_account_info(iter)?;
    let metadata_program = next_account_info(iter)?;
    let token_program    = next_account_info(iter)?;
    let ata_program      = next_account_info(iter)?;
    let system_prog      = next_account_info(iter)?;
    let rent_sysvar      = next_account_info(iter)?;

    require_signer(payer)?;
    require_program(&solana_program::system_program::id(), system_prog.key)?;
    require_program(&METADATA_PROGRAM_ID, metadata_program.key)?;
    require_program(&spl_token::id(), token_program.key)?;
    require_owned_by(coll_pda, program_id)?;

    if proxy_fee_bps > 5000 {
        return Err(NftError::FeeTooHigh.into());
    }

    if kind == NftKind::AddressV4Fraction {
        let total: u32 = fraction_children_raw.iter().map(|(_, s)| *s as u32).sum();
        if total > 10000 {
            return Err(NftError::InvalidFractions.into());
        }
        if fraction_children_raw.len() > 8 {
            return Err(NftError::TooManyFractions.into());
        }
    }

    let mut coll = CollectionState::try_from_slice(&coll_pda.data.borrow())?;
    if coll.minted_count >= coll.total_supply {
        return Err(NftError::CollectionFull.into());
    }

    let mint_index = coll.minted_count;
    let (pda_key, bump) = Pubkey::find_program_address(
        &[b"nft", coll_pda.key.as_ref(), &mint_index.to_le_bytes()],
        program_id,
    );
    require_key(&pda_key, nft_pda.key, NftError::InvalidPda)?;

    let fee_recipient = match kind {
        NftKind::Address | NftKind::AddressV6 => coll.authority,
        NftKind::AddressV5 => *payer.key,
        _ => coll.authority,
    };

    let can_fraction = kind == NftKind::AddressV4Fraction;

    let fraction_children = fraction_children_raw
        .into_iter()
        .map(|(pda, bps)| FractionChild { child_nft_pda: pda, share_bps: bps })
        .collect();

    let state = NftState {
        name: name.clone(),
        symbol: symbol.clone(),
        uri: uri.clone(),
        seller_fee_bps,
        kind,
        collection: *coll_pda.key,
        mint: *mint.key,
        owner: *owner.key, // ← владелец, не payer
        mint_index,
        is_burned: false,
        proxy_target,
        proxy_fee_bps,
        fee_recipient,
        can_fraction,
        fraction_children,
        original_nft: None,
        parent_nft: None,
        generation: 0,
        clone_count: 0,
        pending_rewards_lamports: 0,
        total_received_lamports: 0,
        total_forwarded_lamports: 0,
    };

    // Объявляем seeds первым — используется во всех invoke_signed ниже
    let nft_seeds: &[&[u8]] = &[
        b"nft",
        coll_pda.key.as_ref(),
        &mint_index.to_le_bytes(),
        &[bump],
    ];

    // 1. Создаём PDA аккаунт NftState
    create_pda_account(
        program_id, payer, nft_pda, system_prog,
        &state.try_to_vec()?,
        &[b"nft", coll_pda.key.as_ref(), &mint_index.to_le_bytes(), &[bump]],
    )?;

    // 2. Создаём аккаунт mint
    let rent = Rent::get()?;
    let mint_rent = rent.minimum_balance(spl_token::state::Mint::LEN);
    invoke(
        &system_instruction::create_account(
            payer.key,
            mint.key,
            mint_rent,
            spl_token::state::Mint::LEN as u64,
            token_program.key,
        ),
        &[payer.clone(), mint.clone(), system_prog.clone()],
    )?;

    // 3. Инициализируем SPL Mint (decimals=0 = NFT)
    invoke(
        &spl_token::instruction::initialize_mint(
            token_program.key,
            mint.key,
            nft_pda.key,  // mint_authority = наш PDA
            None,
            0,
        )?,
        &[mint.clone(), rent_sysvar.clone(), token_program.clone()],
    )?;

    // 4. Создаём Metadata (пока mint_authority ещё активен!)
    let metadata_ix = CreateMetadataAccountV3Builder::new()
        .metadata(*metadata_account.key)
        .mint(*mint.key)
        .mint_authority(*nft_pda.key)
        .payer(*payer.key)
        .update_authority(*nft_pda.key, true)
        .data(DataV2 {
            name,
            symbol,
            uri,
            seller_fee_basis_points: seller_fee_bps,
            creators: None,
            collection: None,
            uses: None,
        })
        .is_mutable(true)
        .instruction();

    invoke_signed(
        &metadata_ix,
        &[
            metadata_account.clone(),
            mint.clone(),
            nft_pda.clone(),     // mint_authority
            payer.clone(),       // payer
            nft_pda.clone(),     // update_authority
            system_prog.clone(),
            rent_sysvar.clone(),
            metadata_program.clone(),
        ],
        &[nft_seeds],
    )?;

    // 5. Создаём ATA для владельца
    invoke(
        &spl_associated_token_account::instruction::create_associated_token_account(
            payer.key,
            owner.key,
            mint.key,
            token_program.key,
        ),
        &[
            payer.clone(),
            token_account.clone(),
            owner.clone(),
            mint.clone(),
            system_prog.clone(),
            token_program.clone(),
            ata_program.clone(),
        ],
    )?;

    // 6. Минтим 1 токен на ATA владельца
    invoke_signed(
        &spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            token_account.key,
            nft_pda.key,
            &[],
            1,
        )?,
        &[
            mint.clone(),
            token_account.clone(),
            nft_pda.clone(),
            token_program.clone(),
        ],
        &[nft_seeds],
    )?;

    // 7. Отзываем mint authority (последним!)
    invoke_signed(
        &spl_token::instruction::set_authority(
            token_program.key,
            mint.key,
            None,
            spl_token::instruction::AuthorityType::MintTokens,
            nft_pda.key,
            &[],
        )?,
        &[mint.clone(), nft_pda.clone(), token_program.clone()],
        &[nft_seeds],
    )?;

    // 8. Обновляем счётчик коллекции
    coll.minted_count += 1;
    coll.serialize(&mut *coll_pda.try_borrow_mut_data()?)?;

    msg!("Minted NFT #{} kind={:?} owner={} proxy_target={}", mint_index, state.kind, owner.key, proxy_target);
    Ok(())
}

// ════════════════════════════════════════════════════════
// PROXY FORWARD
// Работает для: Address, AddressV2, AddressV5, AddressV4Fraction
//
// Логика по типам:
//   Address:  накопленные lamports → proxy_target (за вычетом fee → fee_recipient)
//   AddressV2: всё → proxy_target, без fee
//   AddressV5: накопленные → proxy_target, fee → owner NFT
//   V4Fraction: доли → дочерние NFT PDA, остаток → proxy_target
// ════════════════════════════════════════════════════════

fn process_proxy_forward(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let _caller      = next_account_info(iter)?;
    let nft_pda      = next_account_info(iter)?;
    let proxy_target = next_account_info(iter)?;
    let fee_recipient= next_account_info(iter)?;
    let _system_prog = next_account_info(iter)?;

    require_owned_by(nft_pda, program_id)?;
    let mut nft = NftState::try_from_slice(&nft_pda.data.borrow())?;

    // Проверяем что proxy_target совпадает с записанным
    require_key(&nft.proxy_target, proxy_target.key, NftError::WrongProxyTarget)?;

    match nft.kind {
        NftKind::Address | NftKind::AddressV2 |
        NftKind::AddressV5 | NftKind::AddressV4Fraction => {}
        _ => return Err(NftError::WrongNftKind.into()),
    }

    let rent = Rent::get()?;
    let min_balance = rent.minimum_balance(nft_pda.data_len());
    let available = nft_pda
        .lamports()
        .checked_sub(min_balance)
        .ok_or(NftError::InsufficientFunds)?;

    if available == 0 {
        return Err(NftError::NothingToForward.into());
    }

    // V4Fraction — особая логика: распределяем по дочерним NFT
    if nft.kind == NftKind::AddressV4Fraction {
        return process_fraction_forward(
            nft_pda, proxy_target, accounts, &mut nft, available,
        );
    }

    // Address / AddressV2 / AddressV5
    let (fee_amount, target_amount) = if nft.kind == NftKind::AddressV2 {
        (0u64, available)
    } else {
        let fee = available
            .checked_mul(nft.proxy_fee_bps as u64)
            .ok_or(NftError::MathOverflow)?
            .checked_div(10000)
            .ok_or(NftError::MathOverflow)?;
        (fee, available - fee)
    };

    // Прямое списание с PDA (program владеет аккаунтом)
    if target_amount > 0 {
        **nft_pda.try_borrow_mut_lamports()? -= target_amount;
        **proxy_target.try_borrow_mut_lamports()? += target_amount;
    }
    if fee_amount > 0 {
        require_key(&nft.fee_recipient, fee_recipient.key, NftError::WrongFeeRecipient)?;
        **nft_pda.try_borrow_mut_lamports()? -= fee_amount;
        **fee_recipient.try_borrow_mut_lamports()? += fee_amount;
    }

    nft.total_received_lamports =
        nft.total_received_lamports.saturating_add(available);
    nft.total_forwarded_lamports =
        nft.total_forwarded_lamports.saturating_add(available);
    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    msg!(
        "ProxyForward {:?}: {} → target, {} → fee",
        nft.kind, target_amount, fee_amount
    );
    Ok(())
}

/// V4Fraction: распределяем available по долям дочерних NFT,
/// остаток → proxy_target
fn process_fraction_forward<'a>(
    nft_pda: &AccountInfo<'a>,
    proxy_target: &AccountInfo<'a>,
    all_accounts: &[AccountInfo<'a>],
    nft: &mut NftState,
    available: u64,
) -> ProgramResult {
    // accounts[5..] = child NFT PDA в порядке fraction_children
    let child_accounts = &all_accounts[5..];

    if child_accounts.len() != nft.fraction_children.len() {
        return Err(NftError::FractionAccountMismatch.into());
    }

    let mut distributed: u64 = 0;

    for (i, child) in nft.fraction_children.iter().enumerate() {
        require_key(
            &child.child_nft_pda,
            child_accounts[i].key,
            NftError::FractionAccountMismatch,
        )?;

        let share = available
            .checked_mul(child.share_bps as u64)
            .ok_or(NftError::MathOverflow)?
            .checked_div(10000)
            .ok_or(NftError::MathOverflow)?;

        if share > 0 {
            **nft_pda.try_borrow_mut_lamports()? -= share;
            **child_accounts[i].try_borrow_mut_lamports()? += share;

            // Обновляем total_received дочернего NFT
            let mut child_state =
                NftState::try_from_slice(&child_accounts[i].data.borrow())?;
            child_state.total_received_lamports =
                child_state.total_received_lamports.saturating_add(share);
            child_state.serialize(&mut *child_accounts[i].try_borrow_mut_data()?)?;
        }
        distributed = distributed.saturating_add(share);
    }

    // Остаток → proxy_target
    let remainder = available.saturating_sub(distributed);
    if remainder > 0 {
        **nft_pda.try_borrow_mut_lamports()? -= remainder;
        **proxy_target.try_borrow_mut_lamports()? += remainder;
    }

    nft.total_received_lamports = nft.total_received_lamports.saturating_add(available);
    nft.total_forwarded_lamports = nft.total_forwarded_lamports.saturating_add(available);
    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    msg!(
        "FractionForward: {} total, {} to {} children, {} remainder → target",
        available, distributed, nft.fraction_children.len(), remainder
    );
    Ok(())
}

// ════════════════════════════════════════════════════════
// WITHDRAW — статическое хранилище V3 / V3j
// ════════════════════════════════════════════════════════

fn process_withdraw(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let owner       = next_account_info(iter)?;
    let nft_pda     = next_account_info(iter)?;
    let destination = next_account_info(iter)?;
    let _sys        = next_account_info(iter)?;

    require_signer(owner)?;

    let mut nft = NftState::try_from_slice(&nft_pda.data.borrow())?;

    match nft.kind {
        NftKind::AddressV3 | NftKind::AddressV3j => {}
        _ => return Err(NftError::WrongNftKind.into()),
    }

    if nft.owner != *owner.key {
        return Err(NftError::NotOwner.into());
    }

    let rent = Rent::get()?;
    let min_balance = rent.minimum_balance(nft_pda.data_len());
    let available = nft_pda.lamports().saturating_sub(min_balance);

    if available == 0 {
        return Err(NftError::NothingToForward.into()); // или свой NftError::NothingToWithdraw
    }

    **nft_pda.try_borrow_mut_lamports()? -= available;
    **destination.try_borrow_mut_lamports()? += available;

    nft.total_forwarded_lamports =
        nft.total_forwarded_lamports.saturating_add(available);
    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    msg!("Withdraw ALL ({} lamports) from {:?} storage", available, nft.kind);
    Ok(())
}

fn process_withdraw_tokens(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let owner       = next_account_info(iter)?;
    let nft_pda     = next_account_info(iter)?;
    let src_token   = next_account_info(iter)?;
    let dst_token   = next_account_info(iter)?;
    let token_prog  = next_account_info(iter)?;

    require_signer(owner)?;

    let nft = NftState::try_from_slice(&nft_pda.data.borrow())?;

    if nft.kind != NftKind::AddressV3j {
        return Err(NftError::WrongNftKind.into());
    }
    if nft.owner != *owner.key {
        return Err(NftError::NotOwner.into());
    }

    // Читаем реальный баланс src_token, чтобы забрать всё
    let amount = TokenAccount::unpack(&src_token.data.borrow())?.amount;

    if amount == 0 {
        return Err(NftError::NothingToForward.into()); // или свой NftError::NothingToWithdraw
    }

    let (_, bump) = Pubkey::find_program_address(
        &[b"nft", nft.collection.as_ref(), &nft.mint_index.to_le_bytes()],
        _program_id,
    );

    invoke_signed(
        &spl_token::instruction::transfer(
            token_prog.key,
            src_token.key,
            dst_token.key,
            nft_pda.key,
            &[],
            amount,
        )?,
        &[src_token.clone(), dst_token.clone(), nft_pda.clone(), token_prog.clone()],
        &[&[b"nft", nft.collection.as_ref(), &nft.mint_index.to_le_bytes(), &[bump]]],
    )?;

    msg!("WithdrawTokens ALL ({}) from V3j storage", amount);
    Ok(())
}

// ════════════════════════════════════════════════════════
// CLAIM CLONE REWARDS
// ════════════════════════════════════════════════════════

fn process_claim_clone_rewards(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let owner    = next_account_info(iter)?;
    let nft_pda  = next_account_info(iter)?;
    let coll_pda = next_account_info(iter)?; // treasury
    let wallet   = next_account_info(iter)?;

    require_signer(owner)?;

    let mut nft = NftState::try_from_slice(&nft_pda.data.borrow())?;

    match nft.kind {
        NftKind::Clone | NftKind::CloneV2 => {}
        _ => return Err(NftError::WrongNftKind.into()),
    }
    if nft.owner != *owner.key {
        return Err(NftError::NotOwner.into());
    }

    let reward = nft.pending_rewards_lamports;
    if reward == 0 {
        return Err(NftError::NothingToClaim.into());
    }
    if coll_pda.lamports() < reward {
        return Err(NftError::InsufficientFunds.into());
    }

    **coll_pda.try_borrow_mut_lamports()? -= reward;
    **wallet.try_borrow_mut_lamports()? += reward;

    nft.pending_rewards_lamports = 0;
    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    msg!("ClaimRewards: {} lamports", reward);
    Ok(())
}

// ════════════════════════════════════════════════════════
// UPDATE FRACTIONS
// Только owner, только если can_fraction = true
// Сумма долей <= 10000, остаток → proxy_target
// ════════════════════════════════════════════════════════

fn process_update_fractions(
    _program_id: &Pubkey,
    accounts: &[AccountInfo],
    fraction_children_raw: Vec<(Pubkey, u16)>,
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let owner   = next_account_info(iter)?;
    let nft_pda = next_account_info(iter)?;

    require_signer(owner)?;

    let mut nft = NftState::try_from_slice(&nft_pda.data.borrow())?;

    if nft.kind != NftKind::AddressV4Fraction {
        return Err(NftError::WrongNftKind.into());
    }
    if nft.owner != *owner.key {
        return Err(NftError::NotOwner.into());
    }
    if !nft.can_fraction {
        return Err(NftError::CannotFraction.into());
    }

    let total: u32 = fraction_children_raw.iter().map(|(_, s)| *s as u32).sum();
    if total > 10000 {
        return Err(NftError::InvalidFractions.into());
    }
    if fraction_children_raw.len() > 8 {
        return Err(NftError::TooManyFractions.into());
    }

    nft.fraction_children = fraction_children_raw
        .into_iter()
        .map(|(pda, bps)| FractionChild { child_nft_pda: pda, share_bps: bps })
        .collect();

    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    let remainder_bps: u32 = 10000 - total;
    msg!(
        "Fractions updated: {} children, {}bps to children, {}bps to proxy_target",
        nft.fraction_children.len(), total, remainder_bps
    );
    Ok(())
}

// ════════════════════════════════════════════════════════
// AUTO MINT — AddressV6
// Отправитель вызывает инструкцию, платит, получает новый NFT
// ════════════════════════════════════════════════════════

fn process_auto_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_nft_kind: NftKind,
    new_proxy_target: Pubkey,
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let sender       = next_account_info(iter)?;
    let coll_pda     = next_account_info(iter)?;
    let trigger_pda  = next_account_info(iter)?;
    let new_nft_pda  = next_account_info(iter)?;
    let new_mint     = next_account_info(iter)?;
    let fee_recipient= next_account_info(iter)?;
    let system_prog  = next_account_info(iter)?;

    require_signer(sender)?;
    require_program(&solana_program::system_program::id(), system_prog.key)?;
    require_owned_by(coll_pda, program_id)?;

    let mut coll = CollectionState::try_from_slice(&coll_pda.data.borrow())?;
    let trigger = NftState::try_from_slice(&trigger_pda.data.borrow())?;

    if trigger.kind != NftKind::AddressV6 {
        return Err(NftError::WrongNftKind.into());
    }
    require_key(&trigger.fee_recipient, fee_recipient.key, NftError::WrongFeeRecipient)?;

    let price = coll.auto_mint_price_lamports;
    if sender.lamports() < price {
        return Err(NftError::InsufficientFunds.into());
    }

    // fee → владельцу trigger NFT (owner AddressV6)
    let fee = price
        .checked_mul(trigger.proxy_fee_bps as u64)
        .ok_or(NftError::MathOverflow)?
        .checked_div(10000)
        .ok_or(NftError::MathOverflow)?;
    let to_treasury = price - fee;

    if fee > 0 {
        invoke(
            &system_instruction::transfer(sender.key, fee_recipient.key, fee),
            &[sender.clone(), fee_recipient.clone(), system_prog.clone()],
        )?;
    }

    // Остаток → collection PDA (treasury для clone rewards и т.д.)
    invoke(
        &system_instruction::transfer(sender.key, coll_pda.key, to_treasury),
        &[sender.clone(), coll_pda.clone(), system_prog.clone()],
    )?;

    if coll.minted_count >= coll.total_supply {
        return Err(NftError::CollectionFull.into());
    }

    let new_index = coll.minted_count;
    let (new_pda_key, bump) = Pubkey::find_program_address(
        &[b"nft", coll_pda.key.as_ref(), &new_index.to_le_bytes()],
        program_id,
    );
    require_key(&new_pda_key, new_nft_pda.key, NftError::InvalidPda)?;

    let new_nft = NftState {
        name: trigger.name.clone(),
        symbol: trigger.symbol.clone(),
        uri: trigger.uri.clone(),
        seller_fee_bps: trigger.seller_fee_bps,
        kind: new_nft_kind,
        collection: *coll_pda.key,
        mint: *new_mint.key,
        owner: *sender.key,
        mint_index: new_index,
        is_burned: false,
        proxy_target: new_proxy_target,
        proxy_fee_bps: 0,
        fee_recipient: *sender.key,
        can_fraction: false,
        fraction_children: vec![],
        original_nft: None,
        parent_nft: None,
        generation: 0,
        clone_count: 0,
        pending_rewards_lamports: 0,
        total_received_lamports: 0,
        total_forwarded_lamports: 0,
    };

    create_pda_account(
        program_id, sender, new_nft_pda, system_prog,
        &new_nft.try_to_vec()?,
        &[b"nft", coll_pda.key.as_ref(), &new_index.to_le_bytes(), &[bump]],
    )?;

    coll.minted_count += 1;
    coll.serialize(&mut *coll_pda.try_borrow_mut_data()?)?;

    msg!(
        "AutoMint: new NFT #{} kind={:?} for sender {}, fee={} lamports",
        new_index, new_nft.kind, sender.key, fee
    );
    Ok(())
}

// ════════════════════════════════════════════════════════
// ХЕЛПЕРЫ
// ════════════════════════════════════════════════════════

fn require_signer(acc: &AccountInfo) -> ProgramResult {
    if !acc.is_signer { return Err(NftError::Unauthorized.into()); }
    Ok(())
}

fn require_key(expected: &Pubkey, actual: &Pubkey, err: NftError) -> ProgramResult {
    if expected != actual { return Err(err.into()); }
    Ok(())
}

fn require_owned_by(acc: &AccountInfo, owner: &Pubkey) -> ProgramResult {
    if acc.owner != owner {
        return Err(NftError::WrongAccountOwner.into());
    }
    Ok(())
}

fn require_program(expected: &Pubkey, actual: &Pubkey) -> ProgramResult {
    if expected != actual {
        return Err(NftError::WrongProgramId.into());
    }
    Ok(())
}

fn create_pda_account<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    pda: &AccountInfo<'a>,
    system_prog: &AccountInfo<'a>,
    data: &[u8],
    seeds: &[&[u8]],
) -> ProgramResult {
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(data.len());

    if !pda.data_is_empty() || pda.lamports() > 0 {
        return Err(ProgramError::AccountAlreadyInitialized);
    }

    invoke_signed(
        &system_instruction::create_account(
            payer.key, pda.key, lamports, data.len() as u64, program_id,
        ),
        &[payer.clone(), pda.clone(), system_prog.clone()],
        &[seeds],
    )?;

    pda.try_borrow_mut_data()?.copy_from_slice(data);
    Ok(())
}


// ════════════════════════════════════════════════════════
// CLONE NFT
// Оригинал остаётся у owner, новый NFT минтится для recipient
//
// Рекурсия одного типа:
//   generation 0 → клонируется → generation 1 (сохраняет оригинал original_nft)
//   generation 1 → клонируется → generation 2 (parent_nft = gen1, original = gen0)
//
// CloneV2 награды:
//   Если клонируем generation >= 1: gen0 получает 80%, parent(gen1) получает 20%
//   Если клонируем generation 0: весь reward идёт gen0 (= original)
// ════════════════════════════════════════════════════════

fn process_clone_nft(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    recipient: Pubkey,
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let owner        = next_account_info(iter)?;
    let coll_pda     = next_account_info(iter)?;
    let original_pda = next_account_info(iter)?; // NFT который клонируем
    let new_nft_pda  = next_account_info(iter)?;
    let new_mint     = next_account_info(iter)?;
    let system_prog  = next_account_info(iter)?;

    require_signer(owner)?;

    let mut coll = CollectionState::try_from_slice(&coll_pda.data.borrow())?;
    let mut original = NftState::try_from_slice(&original_pda.data.borrow())?;

    if original.owner != *owner.key {
        return Err(NftError::NotOwner.into());
    }
    match original.kind {
        NftKind::Clone | NftKind::CloneV2 => {}
        _ => return Err(NftError::WrongNftKind.into()),
    }
    if coll.minted_count >= coll.total_supply {
        return Err(NftError::CollectionFull.into());
    }

    let new_index = coll.minted_count;
    let (new_pda_key, bump) = Pubkey::find_program_address(
        &[b"nft", coll_pda.key.as_ref(), &new_index.to_le_bytes()],
        program_id,
    );
    require_key(&new_pda_key, new_nft_pda.key, NftError::InvalidPda)?;

    // Определяем original_nft и parent_nft для новой копии
    let (new_original_nft, new_parent_nft) = if original.generation == 0 {
        // Клонируем оригинал → копия знает своего родителя
        (Some(*original_pda.key), None)
    } else {
        // Клонируем копию → original_nft = gen0, parent = текущий
        let gen0 = original.original_nft.unwrap_or(*original_pda.key);
        (Some(gen0), Some(*original_pda.key))
    };

    let new_nft = NftState {
        name: original.name.clone(),
        symbol: original.symbol.clone(),
        uri: original.uri.clone(),
        seller_fee_bps: original.seller_fee_bps,
        kind: original.kind.clone(),
        collection: *coll_pda.key,
        mint: *new_mint.key,
        owner: recipient,
        mint_index: new_index,
        is_burned: false,
        proxy_target: recipient,    // копия проксирует на получателя по умолчанию
        proxy_fee_bps: 0,
        fee_recipient: recipient,
        can_fraction: false,
        fraction_children: vec![],
        original_nft: new_original_nft,
        parent_nft: new_parent_nft,
        generation: original.generation + 1,
        clone_count: 0,
        pending_rewards_lamports: 0,
        total_received_lamports: 0,
        total_forwarded_lamports: 0,
    };

    create_pda_account(
        program_id, owner, new_nft_pda, system_prog,
        &new_nft.try_to_vec()?,
        &[b"nft", coll_pda.key.as_ref(), &new_index.to_le_bytes(), &[bump]],
    )?;

    // Начисляем награды
    original.clone_count += 1;

    if original.kind == NftKind::CloneV2 && original.generation >= 1 {
        // CloneV2: gen0 получает 80%, родитель (generation >= 1) получает 20%
        let reward = coll.clone_reward_lamports;
        let parent_share  = reward * 20 / 100;
        let origin_share  = reward * 80 / 100;

        // Родитель (accounts[2] = original_pda) получает 20%
        original.pending_rewards_lamports += parent_share;

        // Gen0 получает 80% — нужен дополнительный аккаунт
        if let Some(gen0_acc) = accounts.get(6) {
            let mut gen0 = NftState::try_from_slice(&gen0_acc.data.borrow())?;

            // Проверяем что это действительно gen0 оригинал
            let expected_gen0 = original.original_nft.ok_or(NftError::MissingGen0Account)?;
            require_key(&expected_gen0, gen0_acc.key, NftError::MissingGen0Account)?;

            gen0.pending_rewards_lamports += origin_share;
            gen0.serialize(&mut *gen0_acc.try_borrow_mut_data()?)?;
        } else {
            // Gen0 аккаунт не передан — кладём весь reward родителю
            original.pending_rewards_lamports += origin_share;
        }
    } else {
        // Clone v1 или CloneV2 generation=0: весь reward → оригинал
        original.pending_rewards_lamports += coll.clone_reward_lamports;
    }

    original.serialize(&mut *original_pda.try_borrow_mut_data()?)?;

    coll.minted_count += 1;
    coll.serialize(&mut *coll_pda.try_borrow_mut_data()?)?;

    msg!(
        "Cloned NFT #{} → new #{} for {}, generation={}, kind={:?}",
        original.mint_index, new_index, recipient,
        new_nft.generation, new_nft.kind
    );
    Ok(())
}

// ──────────────────────────────────────────
// HANDLER 4: Передача NFT с кастомными проверками
// ──────────────────────────────────────────
fn process_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_owner: Pubkey,
) -> ProgramResult {
    let accounts_iter = &mut accounts.iter();
    let owner = next_account_info(accounts_iter)?;
    let nft_pda = next_account_info(accounts_iter)?;
    let collection_pda = next_account_info(accounts_iter)?;
    let mint               = next_account_info(accounts_iter)?;
    let from_token_account = next_account_info(accounts_iter)?;
    let to_token_account   = next_account_info(accounts_iter)?;
    let new_owner_info     = next_account_info(accounts_iter)?;
    let token_program      = next_account_info(accounts_iter)?;
    let ata_program        = next_account_info(accounts_iter)?;
    let system_prog        = next_account_info(accounts_iter)?;

    if !owner.is_signer {
        return Err(NftError::Unauthorized.into());
    }

    let mut nft = NftState::try_from_slice(&nft_pda.data.borrow())?;
    let collection = CollectionState::try_from_slice(&collection_pda.data.borrow())?;

    if nft.owner != *owner.key {
        return Err(NftError::NotOwner.into());
    }
    if nft.is_burned {
        return Err(NftError::AlreadyBurned.into());
    }

    // SBT (Soul Bound Token) — нельзя передавать никому никогда
    if nft.kind == NftKind::SBT {
        return Err(NftError::SbtNotTransferable.into());
    }
    if nft.kind == NftKind::Clone || nft.kind == NftKind::CloneV2 {
        return process_clone_nft(program_id, accounts, new_owner);
    }
    require_program(&spl_token::id(), token_program.key)?;

     // Создаём ATA для нового владельца если не существует
     if to_token_account.lamports() == 0 {
        invoke(
            &spl_associated_token_account::instruction::create_associated_token_account(
                owner.key,          // плательщик
                new_owner_info.key, // владелец
                mint.key,
                token_program.key,
            ),
            &[
                owner.clone(),
                to_token_account.clone(),
                new_owner_info.clone(),
                mint.clone(),
                system_prog.clone(),
                token_program.clone(),
                ata_program.clone(),
            ],
        )?;
    }

    // Переносим токен
    invoke(
        &spl_token::instruction::transfer(
            token_program.key,
            from_token_account.key,
            to_token_account.key,
            owner.key,
            &[],
            1,
        )?,
        &[
            from_token_account.clone(),
            to_token_account.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )?;

    nft.owner = new_owner;
    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    msg!("NFT #{} transferred to {}", nft.mint_index, new_owner);
    Ok(())
}

fn process_burn_nft(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let owner         = next_account_info(iter)?;
    let nft_pda       = next_account_info(iter)?;
    let coll_pda      = next_account_info(iter)?;
    let mint          = next_account_info(iter)?;
    let token_account = next_account_info(iter)?;
    let token_program = next_account_info(iter)?;

    require_signer(owner)?;
    require_program(&spl_token::id(), token_program.key)?;

    let mut nft = NftState::try_from_slice(&nft_pda.data.borrow())?;

    if nft.owner != *owner.key {
        return Err(NftError::NotOwner.into());
    }
    if nft.is_burned {
        return Err(NftError::AlreadyBurned.into());
    }
    if nft.kind == NftKind::SBT {
        return Err(NftError::SbtNotTransferable.into());
    }

    match nft.kind {
        NftKind::AddressV3 | NftKind::AddressV3j | NftKind::AddressV4Fraction => {
            let rent = Rent::get()?;
            let min_balance = rent.minimum_balance(nft_pda.data_len());
            let available = nft_pda.lamports().saturating_sub(min_balance);
            if available > 0 {
                return Err(NftError::StorageNotEmpty.into());
            }
        }
        _ => {}
    }

    // 1. Сжигаем SPL токен — authority это owner, не PDA
    invoke(
        &spl_token::instruction::burn(
            token_program.key,
            token_account.key,
            mint.key,
            owner.key, // ← владелец подписывает, не PDA
            &[],
            1,
        )?,
        &[
            token_account.clone(),
            mint.clone(),
            owner.clone(), // ← owner как signer
            token_program.clone(),
        ],
    )?;

    // 2. Закрываем token account и возвращаем rent владельцу
    invoke(
        &spl_token::instruction::close_account(
            token_program.key,
            token_account.key,
            owner.key,  // rent получатель
            owner.key,  // authority
            &[],
        )?,
        &[
            token_account.clone(),
            owner.clone(),
            token_program.clone(),
        ],
    )?;

    // 3. Помечаем как сожжённый
    nft.is_burned = true;
    nft.owner = Pubkey::default();
    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    // 4. Возвращаем rent от PDA владельцу
    let nft_lamports = nft_pda.lamports();
    **nft_pda.try_borrow_mut_lamports()? = 0;
    **owner.try_borrow_mut_lamports()? += nft_lamports;

    msg!("NFT #{} burned by {}", nft.mint_index, owner.key);
    Ok(())
}


fn process_update_nft(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    name: Option<String>,
    symbol: Option<String>,
    uri: Option<String>,
    seller_fee_bps: Option<u16>,
    proxy_target: Option<Pubkey>,
    proxy_fee_bps: Option<u16>,
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let authority         = next_account_info(iter)?;
    let nft_pda           = next_account_info(iter)?;
    let metadata_account  = next_account_info(iter)?;
    let metadata_program  = next_account_info(iter)?;
    let coll_pda          = next_account_info(iter)?;

    require_signer(authority)?;

    let mut nft = NftState::try_from_slice(&nft_pda.data.borrow())?;
    let coll = CollectionState::try_from_slice(&coll_pda.data.borrow())?;

    // Только owner или collection authority может обновлять
    if coll.authority != *authority.key {
        return Err(NftError::Unauthorized.into());
    }
    if nft.is_burned {
        return Err(NftError::AlreadyBurned.into());
    }

    let should_update_metadata = name.is_some() 
        || symbol.is_some() 
        || uri.is_some() 
        || seller_fee_bps.is_some();

    // Обновляем NftState
    if let Some(v) = name        { nft.name = v; }
    if let Some(v) = symbol      { nft.symbol = v; }
    if let Some(v) = uri         { nft.uri = v; }
    if let Some(v) = seller_fee_bps { nft.seller_fee_bps = v; }
    if let Some(v) = proxy_target { nft.proxy_target = v; }
    if let Some(v) = proxy_fee_bps {
        if v > 5000 { return Err(NftError::FeeTooHigh.into()); }
        nft.proxy_fee_bps = v;
    }

    nft.serialize(&mut *nft_pda.try_borrow_mut_data()?)?;

    // Обновляем Metaplex метаданные (если меняли name/symbol/uri)
    if should_update_metadata {
        let (_, bump) = Pubkey::find_program_address(
            &[b"nft", nft.collection.as_ref(), &nft.mint_index.to_le_bytes()],
            program_id,
        );

        let update_ix = UpdateV1Builder::new()
            .metadata(*metadata_account.key)
            .authority(*nft_pda.key)
            .data(Data {
                name: nft.name.clone(),
                symbol: nft.symbol.clone(),
                uri: nft.uri.clone(),
                seller_fee_basis_points: nft.seller_fee_bps,
                creators: None,
            })
            .instruction();

        invoke_signed(
            &update_ix,
            &[
                metadata_account.clone(),
                nft_pda.clone(),
                metadata_program.clone(),
            ],
            &[&[
                b"nft",
                nft.collection.as_ref(),
                &nft.mint_index.to_le_bytes(),
                &[bump],
            ]],
        )?;
    }

    msg!("NFT #{} updated", nft.mint_index);
    Ok(())
}

fn process_transfer_collection(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_authority: Pubkey,
) -> ProgramResult {
    let iter = &mut accounts.iter();
    let authority = next_account_info(iter)?;
    let coll_pda  = next_account_info(iter)?;

    require_signer(authority)?;
    require_owned_by(coll_pda, program_id)?;

    let mut coll = CollectionState::try_from_slice(&coll_pda.data.borrow())?;

    if coll.authority != *authority.key {
        return Err(NftError::Unauthorized.into());
    }

    coll.authority = new_authority;
    coll.serialize(&mut *coll_pda.try_borrow_mut_data()?)?;

    msg!("Collection authority transferred to {}", new_authority);
    Ok(())
}