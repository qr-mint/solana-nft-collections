import { Buffer } from "buffer";

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  SystemProgram,
  sendAndConfirmTransaction,
  TransactionInstruction,
} from "@solana/web3.js";
import fs from 'fs';
import { serializeInstruction } from './serializer';
const RPC_URL = process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com";
const connection = new Connection(
  RPC_URL,
  "confirmed"
);
//8i7jTZe9De7cBsMGBEhYFHYP31uaBFD76SnEY5ehkFzk
const authority = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync("./wallet.json", "utf8")))
);

const METADATA_PROGRAM_ID = new PublicKey(
  "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"
);

const { programId: PROGRAM_ID_STR } = JSON.parse(
  fs.readFileSync("./program-id.json", "utf8")
);

const PROGRAM_ID = new PublicKey(PROGRAM_ID_STR);



// ──────────────────────────────────────────
// Хелпер: вычислить PDA (детерминированный адрес)
// ──────────────────────────────────────────

function findNftPda(collectionPda: PublicKey, mintIndex: number) {
  const indexBuf = Buffer.alloc(8);
  indexBuf.writeBigUInt64LE(BigInt(mintIndex));
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("nft"), collectionPda.toBuffer(), indexBuf],
    PROGRAM_ID
  );
  return { pda, bump };
}

// Адрес metadata аккаунта Metaplex (их формула PDA)
function findMetadataPda(mintPubkey: PublicKey) {
  const [pda] = PublicKey.findProgramAddressSync(
    [
      Buffer.from("metadata"),
      METADATA_PROGRAM_ID.toBuffer(),
      mintPubkey.toBuffer(),
    ],
    METADATA_PROGRAM_ID
  );
  return pda;
}

// export const mintNfts = async (mintIndex: number) => {
//   const { pda: collectionPda } = await findCollectionPda();
//   const { pda: nftPda } = await findNftPda(collectionPda, mintIndex);

//   // Создаём новый SPL mint keypair для этого NFT
//   const mintKeypair = Keypair.generate();
//   const mintPubkey = mintKeypair.publicKey;

//   // Associated Token Account (куда упадёт NFT)
//   const tokenAccount = await getAssociatedTokenAddress(
//     mintPubkey,
//     authority.publicKey
//   );

//   // PDA метаданных Metaplex
//   const metadataAccount = await findMetadataPda(mintPubkey);

//   console.log("Mint:", mintPubkey.toBase58());
//   console.log("NFT PDA:", nftPda.toBase58());
//   console.log("Token Account:", tokenAccount.toBase58());
//   console.log("Metadata:", metadataAccount.toBase58());

//   const data = serializeInstruction("MintNft");

//   const instruction = new TransactionInstruction({
//     programId: PROGRAM_ID,
//     keys: [
//       // Порядок ОБЯЗАН совпадать с processor.rs!
//       { pubkey: authority.publicKey,        isSigner: true,  isWritable: true  },
//       { pubkey: collectionPda,              isSigner: false, isWritable: true  },
//       { pubkey: nftPda,                     isSigner: false, isWritable: true  },
//       { pubkey: mintPubkey,                 isSigner: true,  isWritable: true  },
//       { pubkey: tokenAccount,               isSigner: false, isWritable: true  },
//       { pubkey: metadataAccount,            isSigner: false, isWritable: true  },
//       { pubkey: TOKEN_PROGRAM_ID,           isSigner: false, isWritable: false },
//       { pubkey: METADATA_PROGRAM_ID,        isSigner: false, isWritable: false },
//       { pubkey: SystemProgram.programId,    isSigner: false, isWritable: false },
//       { pubkey: SYSVAR_RENT_PUBKEY,         isSigner: false, isWritable: false },
//       { pubkey: ASSOCIATED_TOKEN_PROGRAM_ID,isSigner: false, isWritable: false },
//     ],
//     data,
//   });

//   // Создаём ATA отдельной инструкцией перед минтом
//   const createAtaIx = createAssociatedTokenAccountInstruction(
//     authority.publicKey, // payer
//     tokenAccount,        // ata
//     authority.publicKey, // owner
//     mintPubkey           // mint
//   );

//   const tx = new Transaction()
//     .add(createAtaIx)  // сначала создаём token account
//     .add(instruction); // потом минтим

//   // mintKeypair тоже подписывает — он создаётся в этой транзакции
//   const sig = await sendAndConfirmTransaction(
//     connection,
//     tx,
//     [authority, mintKeypair]
//   );

//   console.log("NFT minted! Tx:", sig);
//   return { mintPubkey, nftPda, tokenAccount };
// }


function readMintedCount(data: Buffer): number {
  const offset = 32 + 8; // после authority и total_supply
  return Number(data.readBigUInt64LE(offset));
}

export async function mintNft(
  params: {
  kind?: string;
  proxyTarget?: PublicKey;
  proxyFeeBps?: number;
}) {
  const COLLECTION_ID = new PublicKey('8i7jTZe9De7cBsMGBEhYFHYP31uaBFD76SnEY5ehkFzk');
  console.log(COLLECTION_ID, 'collectionPda')
  // Читаем текущий mintIndex с блокчейна
  const collectionAccount = await connection.getAccountInfo(COLLECTION_ID);
  if (!collectionAccount) throw new Error("Collection не найдена");

  const mintIndex = readMintedCount(Buffer.from(collectionAccount.data));
  console.log("Mint index:", mintIndex);

  const { pda: nftPda } = findNftPda(COLLECTION_ID, mintIndex);

  // Новый mint keypair для этого NFT
  const mintKeypair = Keypair.generate();

  console.log("NFT PDA:", nftPda.toBase58());
  console.log("Mint:", mintKeypair.publicKey.toBase58());

  const data = serializeInstruction("MintNft", {
    kind: params.kind ?? "Default",
    proxy_target: (params.proxyTarget ?? authority.publicKey).toBase58(),
    proxy_fee_bps: params.proxyFeeBps ?? 0,
    fraction_children: [],
  });

  const instruction = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: authority.publicKey,     isSigner: true,  isWritable: true  },
      { pubkey: COLLECTION_ID,           isSigner: false, isWritable: true  },
      { pubkey: nftPda,                  isSigner: false, isWritable: true  },
      { pubkey: mintKeypair.publicKey,   isSigner: true,  isWritable: true  },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new Transaction().add(instruction);
  const sig = await sendAndConfirmTransaction(
    connection,
    tx,
    [authority, mintKeypair] // оба подписывают
  );

  console.log("NFT minted! Tx:", sig);
  console.log("NFT PDA:", nftPda.toBase58());
  return { nftPda, mintPubkey: mintKeypair.publicKey };
}

enum NftKind {
  Default = "Default",
  SBT = "SBT",
  Address = "Address",           // proxy → произвольный target, fee → collection authority
  AddressV2 = "AddressV2",         // proxy → произвольный target, без fee
  AddressV3 = "AddressV3",         // статическое хранилище SOL, вывод вручную
  AddressV3j = "AddressV3j",        // статическое хранилище SOL + SPL токены
  AddressV4Fraction = "AddressV4Fraction", // дробное: владелец задаёт сколько % уходит дочерним NFT
  AddressV5 = "AddressV5",         // proxy → произвольный target, fee → владельцу NFT
  AddressV6 = "AddressV6",         // авто-минт при входящем платеже
  Clone = "Clone",             // вирусная копия v1 — оригинал остаётся, копия идёт получателю
  CloneV2 = "CloneV2",
}

(async () => {
  await mintNft({
    kind: NftKind.Default
  });
})();