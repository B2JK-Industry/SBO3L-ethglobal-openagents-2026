/**
 * Gateway response signing. CCIP-Read security model:
 *
 * 1. Gateway returns ABI-encoded `(bytes value, uint64 expires, bytes signature)`.
 * 2. OffchainResolver's callback verifies the signature was made by a
 *    pre-registered signer address (baked into the contract at deploy).
 * 3. If the signature checks out, the resolver returns the decoded
 *    `value` to the caller (viem / ethers / any ENSIP-10 client).
 *
 * The signing message hashes (using the ENS Labs reference
 * OffchainResolver convention):
 *
 *     keccak256(0x1900 || resolverAddress || expires || keccak256(callData) || keccak256(result))
 *
 * `0x1900` is the EIP-191 magic byte sequence for "intended validator"
 * (https://eips.ethereum.org/EIPS/eip-191), identifying the resolver
 * contract as the only valid verifier.
 */

import {
  bytesToHex,
  encodeAbiParameters,
  encodePacked,
  keccak256,
  privateKeyToAccount,
  toBytes,
  type Hex,
} from "viem";

const DEFAULT_TTL_SECONDS = 60;

function privateKey(): Hex {
  const raw = process.env.GATEWAY_PRIVATE_KEY;
  if (!raw) {
    throw new Error(
      "GATEWAY_PRIVATE_KEY is not set in the runtime env. " +
        "Set it in Vercel project env (production + preview + development) " +
        "to a fresh secp256k1 hex key. See apps/ccip-gateway/DEPLOY.md.",
    );
  }
  if (!/^0x[0-9a-fA-F]{64}$/.test(raw)) {
    throw new Error(
      "GATEWAY_PRIVATE_KEY must be 0x-prefixed 64-hex-char (32 bytes).",
    );
  }
  return raw as Hex;
}

export function gatewaySignerAddress(): Hex {
  return privateKeyToAccount(privateKey()).address;
}

/**
 * Sign a CCIP-Read gateway response and return the ABI-encoded
 * `(bytes value, uint64 expires, bytes signature)` triple ready to
 * return as the JSON `data` field.
 */
export async function signGatewayResponse(args: {
  resolver: Hex;
  callData: Hex;
  result: Hex;
  ttlSeconds?: number;
}): Promise<{ data: Hex; ttl: number }> {
  const ttl = args.ttlSeconds ?? DEFAULT_TTL_SECONDS;
  const expires = BigInt(Math.floor(Date.now() / 1000) + ttl);

  // EIP-191 "intended validator" digest, ENS Labs OffchainResolver
  // reference impl convention:
  //   keccak256(0x1900 || resolver || expires || keccak256(callData) || keccak256(result))
  const callDataHash = keccak256(args.callData);
  const resultHash = keccak256(args.result);

  const message = encodePacked(
    ["bytes2", "address", "uint64", "bytes32", "bytes32"],
    ["0x1900", args.resolver, expires, callDataHash, resultHash],
  );
  const digest = keccak256(message);

  const account = privateKeyToAccount(privateKey());
  const signature = await account.sign({ hash: digest });

  const data = encodeAbiParameters(
    [
      { type: "bytes" },
      { type: "uint64" },
      { type: "bytes" },
    ],
    [args.result, expires, signature],
  ) as Hex;

  return { data, ttl };
}

/**
 * ABI-encode a string value for return as the `result` field. The
 * OffchainResolver decodes this with `abi.decode(result, (string))`.
 */
export function encodeStringResult(value: string): Hex {
  return encodeAbiParameters([{ type: "string" }], [value]) as Hex;
}

/** ABI-encode an address result for `addr(node)` queries. */
export function encodeAddressResult(addr: Hex): Hex {
  return encodeAbiParameters([{ type: "address" }], [addr]) as Hex;
}

/**
 * Empty result helper — used when a record key is unknown but we still
 * want a valid signed response (PublicResolver convention: missing
 * record returns empty string, not a revert).
 */
export function encodeEmptyStringResult(): Hex {
  return encodeStringResult("");
}

export { bytesToHex, toBytes };
