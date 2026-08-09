import { Keypair, PublicKey } from "@solana/web3.js";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";

export type LocalConfig = {
  sender: string;
  recipient: string;
  operator: string;
  mint: string;
  senderTokenAccount: string;
  recipientTokenAccount: string;
  stream: string;
  escrowAuthority: string;
  escrowTokenAccount: string;
  validator: string;
  streamBump: number;
  escrowBump: number;
  totalAmount?: string;
  chunkAmount?: string;
  intervalMs?: string;
  initialSenderAmount?: string;
  initialRecipientAmount?: string;
};

export function configPath(stateDir: string): string {
  return join(stateDir, "config.json");
}

export async function loadConfig(stateDir: string): Promise<LocalConfig> {
  return JSON.parse(await readFile(configPath(stateDir), "utf8")) as LocalConfig;
}

export async function saveConfig(stateDir: string, config: LocalConfig) {
  await mkdir(stateDir, { recursive: true });
  await writeFile(configPath(stateDir), JSON.stringify(config, null, 2) + "\n");
}

export async function loadOrCreateKeypair(path: string): Promise<Keypair> {
  try {
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(await readFile(path, "utf8"))));
  } catch {
    const keypair = Keypair.generate();
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, JSON.stringify(Array.from(keypair.secretKey)) + "\n");
    return keypair;
  }
}

export const senderPath = (stateDir: string) => join(stateDir, "sender.json");
export const recipientPath = (stateDir: string) => join(stateDir, "recipient.json");
export const operatorPath = (stateDir: string) => join(stateDir, "operator.json");
export const mintPath = (stateDir: string) => join(stateDir, "usdc-mint.json");

export const pk = (value: string) => new PublicKey(value);
