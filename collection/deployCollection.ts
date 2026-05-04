import { Buffer } from "buffer";

import {
  Connection,
  Keypair,
  PublicKey,
  Transaction,
  SystemProgram,
  sendAndConfirmTransaction,
  BpfLoader,
  BPF_LOADER_PROGRAM_ID,
} from "@solana/web3.js";

import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import "dotenv/config";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ── Конфиг ────────────────────────────────────────────
const RPC_URL = process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com";
const connection = new Connection(RPC_URL, "confirmed");

const payerKeypair = Keypair.fromSeed(
  Uint8Array.from(JSON.parse(fs.readFileSync("./wallet.json", "utf8")))
);

export const deployCollection = async () => {
  console.log("Payer:", payerKeypair.publicKey.toBase58());

  // Читаем скомпилированный байткод
  const programData = fs.readFileSync(
    "./target/deploy/nft_collection.so"
  );

  // Генерируем keypair для программы (это и будет ваш program_id)
  const programKeypair = Keypair.generate();
  // Или загружаем существующий: Keypair.fromSecretKey(...)

  console.log("Program ID:", programKeypair.publicKey.toBase58());

  // Деплоим через BpfLoader
  await BpfLoader.load(
    connection,
    payerKeypair,
    programKeypair,
    programData,
    BPF_LOADER_PROGRAM_ID
  );

  console.log("Program deployed successfully!");

  // Сохраняем program_id для дальнейшего использования
  fs.writeFileSync(
    "./program-id.json",
    JSON.stringify({ programId: programKeypair.publicKey.toBase58() })
  );

  return programKeypair.publicKey;
};