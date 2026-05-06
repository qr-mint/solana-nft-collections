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

const authority = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync("./wallet.json", "utf8")))
);

const METADA_PROGRAM_ID = new PublicKey(
  "5hU8pdP9xcchEKuvFUxCAZEvwdaEmMjHwRwSA8kN5ChD"
);

const { programId: PROGRAM_ID_STR } = JSON.parse(
  fs.readFileSync("./program-id.json", "utf8")
);

const PROGRAM_ID = new PublicKey(PROGRAM_ID_STR);

export const initCollection = async () => {
  const { pda: collectionPda } = await findCollectionPda();

  console.log("Collection PDA:", collectionPda.toBase58());

  const data = serializeInstruction("InitializeCollection", {
    uriBase: "https://api.qr-mint.net/images/collections/cover_fb31395e-ac00-4567-a04d-597603e5ae53.webp",
    clone_reward_lamports: 0,
    auto_mint_price_lamports: 0,
    total_supply: 1000,
  });

  const instruction = new TransactionInstruction({
    programId: PROGRAM_ID,
    keys: [
      { pubkey: authority.publicKey, isSigner: true, isWritable: true },
      { pubkey: collectionPda,       isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  });

  const tx = new Transaction().add(instruction);
  const sig = await sendAndConfirmTransaction(connection, tx, [authority]);

  console.log("Collection initialized! Tx:", sig);
  console.log("Collection PDA:", collectionPda.toBase58());
  return collectionPda;
};

async function findCollectionPda() {
  const [pda, bump] = PublicKey.findProgramAddressSync(
    [Buffer.from("collection"), authority.publicKey.toBuffer()],
    PROGRAM_ID
  );
  return { pda, bump };
};

initCollection();