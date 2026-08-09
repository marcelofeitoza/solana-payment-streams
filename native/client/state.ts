import { Keypair } from "@solana/web3.js";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname } from "node:path";

export const STATE_FILE = process.env.STATE_FILE ?? ".local/node-client.json";

export type LocalState = {
  bootstrap: string;
  sender: string;
  recipient: string;
  keeper: string;
  mint: string;
  sourceToken: string;
  stream?: string;
  vault?: string;
  recipientToken?: string;
};

export async function loadState(): Promise<LocalState> {
  return JSON.parse(await readFile(STATE_FILE, "utf8")) as LocalState;
}

export async function saveState(state: LocalState) {
  await mkdir(dirname(STATE_FILE), { recursive: true });
  await writeFile(STATE_FILE, JSON.stringify(state, null, 2) + "\n");
}

export async function loadOrCreate(path: string): Promise<Keypair> {
  try {
    return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(await readFile(path, "utf8"))));
  } catch {
    const keypair = Keypair.generate();
    await mkdir(dirname(path), { recursive: true });
    await writeFile(path, JSON.stringify(Array.from(keypair.secretKey)) + "\n");
    return keypair;
  }
}
