import { PublicKey } from "@solana/web3.js";

export function serializeInstruction(variant: string, fields: any = {}): Buffer {
  const parts: Buffer[] = [];

  // Порядок ОБЯЗАН совпадать с enum NftInstruction в instruction.rs
  const variantMap: Record<string, number> = {
    InitializeCollection: 0,
    MintNft:              1,
    ProxyForward:         2,
    Withdraw:             3,
    WithdrawTokens:       4,
    CloneNft:             5,
    ClaimCloneRewards:    6,
    UpdateFractions:      7,
    AutoMint:             8,
    BurnNft:              9,
  };

  const idx = variantMap[variant];
  if (idx === undefined) throw new Error(`Unknown variant: ${variant}`);

  parts.push(Buffer.from([idx]));

  if (variant === "InitializeCollection") {
    const totalSupply = Buffer.alloc(8);
    totalSupply.writeBigUInt64LE(BigInt(fields.total_supply ?? 0));
    parts.push(totalSupply);

    const uriBytes = Buffer.from(fields.uriBase ?? "", "utf8");
    const uriLen = Buffer.alloc(4);
    uriLen.writeUInt32LE(uriBytes.length);
    parts.push(uriLen);
    parts.push(uriBytes);

    const cloneReward = Buffer.alloc(8);
    cloneReward.writeBigUInt64LE(BigInt(fields.clone_reward_lamports ?? 0));
    parts.push(cloneReward);

    const autoMint = Buffer.alloc(8);
    autoMint.writeBigUInt64LE(BigInt(fields.auto_mint_price_lamports ?? 0));
    parts.push(autoMint);
  }

  if (variant === "MintNft") {
 
    const nameBytes = Buffer.from(fields.name ?? "", "utf8");
    const nameLen = Buffer.alloc(4);
    nameLen.writeUInt32LE(nameBytes.length);
    parts.push(nameLen);
    parts.push(nameBytes);

    const symbolBytes = Buffer.from(fields.symbol ?? "", "utf8");
    const symbolLen = Buffer.alloc(4);
    symbolLen.writeUInt32LE(symbolBytes.length);
    parts.push(symbolLen);
    parts.push(symbolBytes);

    const uriBytes = Buffer.from(fields.uri ?? "", "utf8");
    const uriLen = Buffer.alloc(4);
    uriLen.writeUInt32LE(uriBytes.length);
    parts.push(uriLen);
    parts.push(uriBytes);

    const sellerBps = Buffer.alloc(2);
    sellerBps.writeUInt16LE(fields.seller_fee_bps ?? 0);
    parts.push(sellerBps);
    // kind: u8 — индекс NftKind enum
    // Порядок ОБЯЗАН совпадать с enum NftKind в state.rs
    const kindMap: Record<string, number> = {
      Default:           0,
      SBT:               1,
      Address:           2,
      AddressV2:         3,
      AddressV3:         4,
      AddressV3j:        5,
      AddressV4Fraction: 6,
      AddressV5:         7,
      AddressV6:         8,
      Clone:             9,
      CloneV2:           10,
    };

    const kindIdx = kindMap[fields.kind ?? "Default"];
    if (kindIdx === undefined) throw new Error(`Unknown kind: ${fields.kind}`);
    parts.push(Buffer.from([kindIdx]));

    // proxy_target: Pubkey (32 байта)
    const proxyTarget = new PublicKey(fields.proxy_target);
    parts.push(proxyTarget.toBuffer());

    // proxy_fee_bps: u16 (2 байта LE)
    const feeBps = Buffer.alloc(2);
    feeBps.writeUInt16LE(fields.proxy_fee_bps ?? 0);
    parts.push(feeBps);

    // fraction_children: Vec<(Pubkey, u16)>
    // u32 длина + элементы
    const children: Array<{ pda: string; bps: number }> =
      fields.fraction_children ?? [];
    const vecLen = Buffer.alloc(4);
    vecLen.writeUInt32LE(children.length);
    parts.push(vecLen);

    for (const child of children) {
      parts.push(new PublicKey(child.pda).toBuffer());
      const bps = Buffer.alloc(2);
      bps.writeUInt16LE(child.bps);
      parts.push(bps);
    }
  }

  if (variant === "Withdraw") {
    const amount = Buffer.alloc(8);
    amount.writeBigUInt64LE(BigInt(fields.amount_lamports ?? 0));
    parts.push(amount);
  }

  if (variant === "WithdrawTokens") {
    const amount = Buffer.alloc(8);
    amount.writeBigUInt64LE(BigInt(fields.amount ?? 0));
    parts.push(amount);
  }

  if (variant === "CloneNft") {
    parts.push(new PublicKey(fields.recipient).toBuffer());
  }

  if (variant === "UpdateFractions") {
    const children: Array<{ pda: string; bps: number }> =
      fields.fraction_children ?? [];
    const vecLen = Buffer.alloc(4);
    vecLen.writeUInt32LE(children.length);
    parts.push(vecLen);

    for (const child of children) {
      parts.push(new PublicKey(child.pda).toBuffer());
      const bps = Buffer.alloc(2);
      bps.writeUInt16LE(child.bps);
      parts.push(bps);
    }
  }

  if (variant === "AutoMint") {
    const kindMap: Record<string, number> = {
      Default: 0, SBT: 1, Address: 2, AddressV2: 3,
      AddressV3: 4, AddressV3j: 5, AddressV4Fraction: 6,
      AddressV5: 7, AddressV6: 8, Clone: 9, CloneV2: 10,
    };
    parts.push(Buffer.from([kindMap[fields.new_nft_kind] ?? 0]));
    parts.push(new PublicKey(fields.new_proxy_target).toBuffer());
  }

  if (variant === "TransferNft") {
    // Pubkey = 32 байта
    parts.push(fields.newOwner.toBuffer());
  }
  // ProxyForward, ClaimCloneRewards — только индекс, нет полей

  return Buffer.concat(parts);
}

//2fyiMYt7QzHQuNZe74VKXC8kBom1uu3rQnYbT86zFn4X