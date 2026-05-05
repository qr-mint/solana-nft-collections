import { execSync } from "child_process";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";
import "dotenv/config";
//7e8noEp6WFHCNuyPThYF3b7fL7vPS5Fj1zam4JFsQkwX
//1.03354912 solana

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const RPC_URL = process.env.SOLANA_RPC_URL ?? "https://api.devnet.solana.com";

export const deployProgram = (): PublicKey => {
  const soPath = path.resolve(__dirname, "../collection/target/deploy/nft_collection.so");
  const keypairPath = path.resolve(__dirname, "../program-keypair.json");
  const walletPath = path.resolve(__dirname, "../wallet.json");

  if (!fs.existsSync(soPath)) {
    throw new Error(`Файл не найден: ${soPath}\nЗапусти: cargo build-sbf`);
  }

  // Генерируем keypair если нет
  if (!fs.existsSync(keypairPath)) {
    console.log("Generating program keypair...");
    execSync(`solana-keygen new --outfile ${keypairPath} --no-bip39-passphrase`);
  }

  console.log("Deploying program via Solana CLI...");

  // Деплой через CLI
  const output = execSync(
    `solana program deploy ${soPath} \
     --program-id ${keypairPath} \
     --keypair ${walletPath} \
     --url ${RPC_URL} \
     --output json`,
    { encoding: "utf8" }
  );

  const result = JSON.parse(output);
  const programId = new PublicKey(result.programId);

  console.log("✅ Program deployed:", programId.toBase58());

  // Сохраняем program_id
  fs.writeFileSync(
    path.resolve(__dirname, "../program-id.json"),
    JSON.stringify({ programId: programId.toBase58(), network: RPC_URL }, null, 2)
  );

  return programId;
};

deployProgram();
