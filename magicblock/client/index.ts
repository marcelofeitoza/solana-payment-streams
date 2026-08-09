import { Connection, PublicKey } from "@solana/web3.js";

import { LOCAL_VALIDATOR_STR } from "./addresses.js";
import {
  balances,
  bootstrapLocalDlp,
  initStream,
  initUsdc,
  mint,
  printBalances,
  schedule,
  watch,
} from "./app.js";

const BASE_RPC = process.env.BASE_RPC ?? "http://127.0.0.1:9900";
const ER_RPC = process.env.ER_RPC ?? "http://127.0.0.1:17899";
const STATE_DIR = process.env.STATE_DIR ?? ".local";

function flag(name: string, fallback?: string): string {
  const index = process.argv.indexOf(`--${name}`);
  const value = index === -1 ? undefined : process.argv[index + 1];
  if (value !== undefined) return value;
  if (fallback !== undefined) return fallback;
  throw new Error(`missing --${name}`);
}

async function main() {
  const base = new Connection(BASE_RPC, "confirmed");
  const ephemeral = new Connection(ER_RPC, "confirmed");
  const action = process.argv[2] ?? "demo";

  if (action === "bootstrap-local-dlp") {
    const signatures = await bootstrapLocalDlp(base);
    if (signatures.length === 0) console.log("local DLP fee accounts already initialized");
    else for (const signature of signatures) console.log(`signature=${signature}`);
    return;
  }
  if (action === "init-usdc") {
    const validator = new PublicKey(flag("validator", LOCAL_VALIDATOR_STR));
    const config = await initUsdc(base, STATE_DIR, validator);
    console.log(`mint=${config.mint}`);
    console.log(`sender=${config.sender}`);
    console.log(`recipient=${config.recipient}`);
    console.log(`sender_token=${config.senderTokenAccount}`);
    console.log(`recipient_token=${config.recipientTokenAccount}`);
    console.log(`escrow_token=${config.escrowTokenAccount}`);
    console.log(`stream=${config.stream}`);
    return;
  }
  if (action === "mint") {
    const amount = BigInt(flag("amount", "1000000"));
    const { signature, balance } = await mint(base, STATE_DIR, amount);
    console.log(`signature=${signature}`);
    console.log(`sender_balance=${balance}`);
    return;
  }
  if (action === "init-stream") {
    const result = await initStream(
      base,
      STATE_DIR,
      BigInt(flag("total", "1000000")),
      BigInt(flag("chunk", "100")),
      BigInt(flag("interval-ms", "10")),
      BigInt(flag("fee-reserve-lamports", "5000000000")),
    );
    console.log(`signature=${result.signature}`);
    console.log(`total_base_units=${result.totalAmount}`);
    console.log(`chunk_base_units=${result.chunkAmount}`);
    console.log(`iterations=${result.iterations}`);
    return;
  }
  if (action === "schedule") {
    console.log(`signature=${await schedule(base, ephemeral, STATE_DIR)}`);
    return;
  }
  if (action === "watch") {
    const timeoutSeconds = Number(flag("timeout-seconds", "600"));
    const report = await watch(base, STATE_DIR, timeoutSeconds);
    console.log(`executions=${report.executions}`);
    printBalances(report.finalBalances);
    return;
  }
  if (action === "balances") {
    printBalances(await balances(base, STATE_DIR));
    return;
  }
  if (action === "demo") {
    await initUsdc(base, STATE_DIR, new PublicKey(LOCAL_VALIDATOR_STR));
    await mint(base, STATE_DIR, 1_000_000n);
    printBalances(await balances(base, STATE_DIR));
    await initStream(base, STATE_DIR, 1_000_000n, 100n, 1n, 5_000_000_000n);
    printBalances(await balances(base, STATE_DIR));
    await schedule(base, ephemeral, STATE_DIR);
    const report = await watch(base, STATE_DIR, 900);
    console.log(`executions=${report.executions}`);
    printBalances(report.finalBalances);
    return;
  }
  throw new Error(`unknown action: ${action}`);
}

await main();
