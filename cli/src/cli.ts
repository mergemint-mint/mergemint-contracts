#!/usr/bin/env node

import { MergeMintSDK, createNetworkConfig, TESTNET } from "@mergemint/sdk";

interface CliConfig {
  network: "testnet" | "mainnet";
  contractId: string;
  rpcUrl?: string;
  json: boolean;
  dryRun: boolean;
  secretEnv?: string;
}

const NETWORKS = ["testnet", "mainnet"] as const;

function bigintReplacer(_key: string, value: unknown): unknown {
  return typeof value === "bigint" ? value.toString() : value;
}

function printJson(value: unknown): void {
  console.log(JSON.stringify(value, bigintReplacer, 2));
}

function requireFlagValue(args: string[], i: number, flag: string): string {
  const value = args[i + 1];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`Missing value for ${flag}`);
  }
  return value;
}

function parseNetwork(value: string): "testnet" | "mainnet" {
  if (!(NETWORKS as readonly string[]).includes(value)) {
    throw new Error(
      `Invalid network: ${value}. Expected one of: ${NETWORKS.join(", ")}`
    );
  }
  return value as "testnet" | "mainnet";
}

function parseArgs(argv: string[] = process.argv.slice(2)): {
  command: string;
  args: string[];
  config: CliConfig;
} {
  const config: CliConfig = {
    network: parseNetwork(process.env.MERGEMINT_NETWORK ?? "testnet"),
    contractId: process.env.MERGEMINT_CONTRACT_ID ?? "",
    rpcUrl: process.env.MERGEMINT_RPC_URL,
    json: false,
    dryRun: false,
    secretEnv: undefined,
  };

  let command = "";
  const commandArgs: string[] = [];

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];

    if (arg === "--network") {
      config.network = parseNetwork(requireFlagValue(argv, i, arg));
      i++;
    } else if (arg === "--contract-id") {
      config.contractId = requireFlagValue(argv, i, arg);
      i++;
    } else if (arg === "--rpc-url") {
      config.rpcUrl = requireFlagValue(argv, i, arg);
      i++;
    } else if (arg === "--secret-env") {
      config.secretEnv = requireFlagValue(argv, i, arg);
      i++;
    } else if (arg === "--dry-run") {
      config.dryRun = true;
    } else if (arg === "--json") {
      config.json = true;
    } else if (arg === "--help" || arg === "-h") {
      showHelp();
      process.exit(0);
    } else if (!command) {
      command = arg;
    } else {
      commandArgs.push(arg);
    }
  }

  return { command, args: commandArgs, config };
}

function showHelp(): void {
  console.log(`
MergeMint CLI

Usage:
  mergemint <command> [options]

Commands:
  list [creator]      List bounties (optionally filter by creator)
  get <bounty-id>     Get a specific bounty
  create <params>     Create a new bounty (JSON params)
  cancel <bounty-id>  Cancel a bounty
  claim <bounty-id>   Claim a bounty
  submit <bounty-id>  Submit work for a bounty
  complete <bounty-id> Complete a bounty
  status <bounty-id>  Show bounty status

Options:
  --network <name>    Network: testnet or mainnet (default: MERGEMINT_NETWORK or testnet)
  --contract-id <id>  Contract ID (default: MERGEMINT_CONTRACT_ID)
  --rpc-url <url>     RPC URL (default: MERGEMINT_RPC_URL or network default)
  --secret-env <name> Env var holding the signer secret (never pass secrets on argv)
  --dry-run           Simulate and print XDR without submitting
  --json              Output as JSON
  --help, -h          Show this help

Environment Variables:
  MERGEMINT_NETWORK    Target network
  MERGEMINT_CONTRACT_ID  Contract address
  MERGEMINT_RPC_URL    Soroban RPC endpoint

Examples:
  mergemint list
  mergemint list GABC...
  mergemint get abc123def...
  mergemint create '{"creator":"GABC...","title":"Bounty","rewardAmount":"1000000",...}'
  mergemint claim abc123def... --secret-env MERGEMINT_SECRET
  `);
}

function resolveSigner(config: CliConfig): string | undefined {
  if (!config.secretEnv) {
    return undefined;
  }
  const secret = process.env[config.secretEnv];
  if (!secret) {
    throw new Error(
      `Environment variable ${config.secretEnv} is not set (required by --secret-env)`
    );
  }
  return secret;
}

async function executeCommand(
  command: string,
  args: string[],
  config: CliConfig
): Promise<void> {
  // Validate config
  if (!config.contractId) {
    throw new Error(
      "Contract ID is required. Set MERGEMINT_CONTRACT_ID or pass --contract-id"
    );
  }

  // Create SDK instance
  const networkConfig =
    config.network === "mainnet"
      ? { rpcUrl: config.rpcUrl || "https://mainnet.stellar.validationcloud.io", contractId: config.contractId, networkPassphrase: "Public Global Stellar Network ; September 2015" }
      : createNetworkConfig(TESTNET, config.contractId, config.rpcUrl ? { rpcUrl: config.rpcUrl } : undefined);

  const sdk = new MergeMintSDK(networkConfig);

  switch (command) {
    case "list": {
      const creator = args[0];
      if (creator) {
        const bountyIds = await sdk.getBountiesByCreator(creator);
        if (config.json) {
          printJson({ bountyIds });
        } else {
          console.log(`Bounties by ${creator}:`);
          bountyIds.forEach((id) => console.log(`  ${id}`));
        }
      } else {
        const count = await sdk.getBountyCount();
        if (config.json) {
          printJson({ totalBounties: count.toString() });
        } else {
          console.log(`Total bounties: ${count}`);
        }
      }
      break;
    }

    case "get": {
      const bountyId = args[0];
      if (!bountyId) {
        throw new Error("Bounty ID is required");
      }
      const bounty = await sdk.getBounty(bountyId);
      const meta = await sdk.getBountyMeta(bountyId);
      const result = { ...bounty, ...meta };
      if (config.json) {
        printJson(result);
      } else {
        printJson(result);
      }
      break;
    }

    case "status": {
      const bountyId = args[0];
      if (!bountyId) {
        throw new Error("Bounty ID is required");
      }
      const bounty = await sdk.getBounty(bountyId);
      const meta = await sdk.getBountyMeta(bountyId);
      const result = { bountyId, ...bounty, ...meta };
      if (config.json) {
        printJson(result);
      } else {
        console.log(`Bounty ${bountyId}:`);
        printJson(result);
      }
      break;
    }

    case "create": {
      const paramsJson = args[0];
      if (!paramsJson) {
        throw new Error("Parameters JSON is required");
      }
      const params = JSON.parse(paramsJson);
      const signer = resolveSigner(config);
      const result = await sdk.createBounty(params, {
        signer,
        simulate: config.dryRun || !signer,
      });
      if (config.json) {
        printJson(result);
      } else {
        console.log(
          config.dryRun || !signer
            ? "Bounty creation simulated (dry-run):"
            : "Bounty created:"
        );
        printJson(result);
      }
      break;
    }

    case "cancel": {
      const bountyId = args[0];
      if (!bountyId) {
        throw new Error("Bounty ID is required");
      }
      const signer = resolveSigner(config);
      const result = await sdk.cancelBounty(bountyId, {
        signer,
        simulate: config.dryRun || !signer,
      });
      if (config.json) {
        printJson(result);
      } else {
        console.log(
          config.dryRun || !signer
            ? `Cancellation of bounty ${bountyId} simulated (dry-run):`
            : `Bounty ${bountyId} cancelled:`
        );
        printJson(result);
      }
      break;
    }

    case "claim": {
      const bountyId = args[0];
      if (!bountyId) {
        throw new Error("Bounty ID is required");
      }
      const signer = resolveSigner(config);
      const result = await sdk.claimBounty(bountyId, {
        signer,
        simulate: config.dryRun || !signer,
      });
      if (config.json) {
        printJson(result);
      } else {
        console.log(
          config.dryRun || !signer
            ? `Claim of bounty ${bountyId} simulated (dry-run):`
            : `Bounty ${bountyId} claimed:`
        );
        printJson(result);
      }
      break;
    }

    case "submit": {
      const bountyId = args[0];
      if (!bountyId) {
        throw new Error("Bounty ID is required");
      }
      const signer = resolveSigner(config);
      const result = await sdk.submitWork(bountyId, {
        signer,
        simulate: config.dryRun || !signer,
      });
      if (config.json) {
        printJson(result);
      } else {
        console.log(
          config.dryRun || !signer
            ? `Submission for bounty ${bountyId} simulated (dry-run):`
            : `Work submitted for bounty ${bountyId}:`
        );
        printJson(result);
      }
      break;
    }

    case "complete": {
      const bountyId = args[0];
      if (!bountyId) {
        throw new Error("Bounty ID is required");
      }
      const signer = resolveSigner(config);
      const result = await sdk.completeBounty(bountyId, {
        signer,
        simulate: config.dryRun || !signer,
      });
      if (config.json) {
        printJson(result);
      } else {
        console.log(
          config.dryRun || !signer
            ? `Completion of bounty ${bountyId} simulated (dry-run):`
            : `Bounty ${bountyId} completed:`
        );
        printJson(result);
      }
      break;
    }

    default:
      throw new Error(`Unknown command: ${command}`);
  }
}

async function main(): Promise<void> {
  try {
    const { command, args, config } = parseArgs();

    if (!command) {
      showHelp();
      process.exit(1);
    }

    await executeCommand(command, args, config);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Error: ${message}`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

export { parseArgs, executeCommand, bigintReplacer, parseNetwork };
