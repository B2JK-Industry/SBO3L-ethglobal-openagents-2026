//! T-5-1 — Uniswap full swap construction (Sepolia).
//!
//! Extends `uniswap_live` (quote-only) with **swap calldata construction**
//! for `SwapRouter02.exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))`.
//! Picks SwapRouter02 over the Universal Router so the demo path is one
//! transaction with **no Permit2 dance** — agent calls
//! `IERC20(tokenIn).approve(SwapRouter02, amount)` once, then
//! `SwapRouter02.exactInputSingle(...)` per swap. Permit2 (and the
//! Universal Router) is a follow-up; the simpler path is the right
//! v1 demo for "agent does a real on-chain swap through SBO3L".
//!
//! What this module ships:
//!
//! 1. [`SwapParams`] — parameter struct mirroring SwapRouter02's tuple arg.
//! 2. [`encode_exact_input_single`] — pure function returning the 4-byte
//!    selector + ABI-encoded `(address,address,uint24,address,uint256,uint256,uint160)`
//!    payload, ready to drop into `eth_sendRawTransaction`'s `data:` field.
//! 3. Sepolia constants ([`SEPOLIA_SWAP_ROUTER_02`], [`SEPOLIA_USDC`])
//!    so `SwapParams::sepolia_default_eth_for_usdc(...)` builds a
//!    runnable swap with no operator-supplied addresses needed.
//! 4. The selector is **derived in tests** from the canonical type
//!    string via keccak256, never hardcoded without the pin — drift
//!    breaks the live integration loudly.
//!
//! What this module deliberately does NOT do:
//!
//! - Sign or broadcast. Signing happens in the SDK helpers (TS via
//!   `viem`, Py via `web3.py`) so the no-key boundary stays intact:
//!   the daemon never sees a private key. The Rust module is a
//!   pure-function calldata builder. Callers compose with their
//!   preferred signer + RPC.
//! - Approve `IERC20(tokenIn).approve(SwapRouter02, amount)`. That's a
//!   separate ERC-20 call the agent emits before the first swap. The
//!   SDK helpers may do it as part of `swap()` if a `--auto-approve`
//!   flag is passed.
//!
//! Truthfulness rules (mirror `uniswap_live`):
//!
//! - Sepolia SwapRouter02 address (`0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E`)
//!   is documented at `developers.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments`.
//! - Sepolia USDC address (`0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238`) is the
//!   official Circle Sepolia USDC; surfaced as the default `token_out`.
//! - Selector pinned in tests against `keccak256("exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))")[0..4]`.

use crate::uniswap_live::{SEPOLIA_CHAIN_ID, SEPOLIA_WETH};

/// Sepolia SwapRouter02 deployment address.
///
/// Source: `developers.uniswap.org/contracts/v3/reference/deployments/ethereum-deployments`.
pub const SEPOLIA_SWAP_ROUTER_02: &str = "0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E";

/// Sepolia USDC (Circle's official testnet USDC). 6 decimals.
pub const SEPOLIA_USDC: &str = "0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238";

/// Selector for `exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))`.
///
/// Pinned in tests against `keccak256(canonical_type_string)[0..4]`.
pub const EXACT_INPUT_SINGLE_SELECTOR: [u8; 4] = [0x04, 0xe4, 0x5a, 0xaf];

/// Re-export Sepolia chain id so callers don't need a separate import.
pub use crate::uniswap_live::SEPOLIA_CHAIN_ID as SEPOLIA_CHAIN_ID_TRADING;

/// Parameter set for `SwapRouter02.exactInputSingle`. Mirrors the on-chain
/// `ExactInputSingleParams` struct one-for-one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwapParams {
    /// `address tokenIn` — the token being sold (e.g. WETH).
    pub token_in: [u8; 20],
    /// `address tokenOut` — the token being bought (e.g. USDC).
    pub token_out: [u8; 20],
    /// `uint24 fee` — pool fee tier in hundredths of a bip. 500 / 3000 / 10000.
    pub fee: u32,
    /// `address recipient` — receives `tokenOut`. Usually `msg.sender`.
    pub recipient: [u8; 20],
    /// `uint256 amountIn` — exact amount of `tokenIn` to spend. Wei units of `tokenIn`.
    pub amount_in: [u8; 32],
    /// `uint256 amountOutMinimum` — slippage floor. Set from a recent quote, e.g. `quote * 0.99`.
    pub amount_out_minimum: [u8; 32],
    /// `uint160 sqrtPriceLimitX96` — price ceiling. `0` disables the limit (most common).
    pub sqrt_price_limit_x96: [u8; 32],
}

impl SwapParams {
    /// Build a Sepolia WETH → USDC swap with sensible defaults: 0.3% fee tier,
    /// no price limit. `recipient` is the caller's EOA. `amount_in` is Wei
    /// (so 1 WETH = 1_000_000_000_000_000_000).
    pub fn sepolia_weth_for_usdc(
        recipient: [u8; 20],
        amount_in_wei: u128,
        amount_out_minimum_usdc_micros: u128,
    ) -> Result<Self, AddressError> {
        let token_in = parse_address(SEPOLIA_WETH)?;
        let token_out = parse_address(SEPOLIA_USDC)?;
        Ok(Self {
            token_in,
            token_out,
            fee: 3_000,
            recipient,
            amount_in: u128_to_u256_be(amount_in_wei),
            amount_out_minimum: u128_to_u256_be(amount_out_minimum_usdc_micros),
            sqrt_price_limit_x96: [0u8; 32],
        })
    }

    /// Mirror of `sepolia_weth_for_usdc` but USDC → WETH.
    pub fn sepolia_usdc_for_weth(
        recipient: [u8; 20],
        amount_in_micros: u128,
        amount_out_minimum_wei: u128,
    ) -> Result<Self, AddressError> {
        let token_in = parse_address(SEPOLIA_USDC)?;
        let token_out = parse_address(SEPOLIA_WETH)?;
        Ok(Self {
            token_in,
            token_out,
            fee: 3_000,
            recipient,
            amount_in: u128_to_u256_be(amount_in_micros),
            amount_out_minimum: u128_to_u256_be(amount_out_minimum_wei),
            sqrt_price_limit_x96: [0u8; 32],
        })
    }
}

/// Errors decoding hex-shaped addresses.
#[derive(Debug, thiserror::Error)]
pub enum AddressError {
    /// Length wasn't 42 chars (`0x` + 40 hex). Surface what we got so the
    /// operator can spot a copy-paste mistake at a glance.
    #[error("address must be 0x-prefixed 40-hex (42 chars total), got {0} chars")]
    BadLength(usize),
    /// Non-hex byte inside the address.
    #[error("address contains non-hex character: {0}")]
    NonHex(char),
}

/// Parse a `0x...` address into `[u8; 20]`.
pub fn parse_address(s: &str) -> Result<[u8; 20], AddressError> {
    let trimmed = s.strip_prefix("0x").unwrap_or(s);
    if trimmed.len() != 40 {
        return Err(AddressError::BadLength(trimmed.len()));
    }
    let mut out = [0u8; 20];
    for (i, chunk) in trimmed.as_bytes().chunks(2).enumerate() {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8, AddressError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(AddressError::NonHex(b as char)),
    }
}

fn u128_to_u256_be(v: u128) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[16..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Build the full ABI-encoded calldata for
/// `exactInputSingle(ExactInputSingleParams)`.
///
/// Layout:
///
/// ```text
///   0..4    : selector (0x04 0xe4 0x5a 0xaf)
///   4..36   : tokenIn   (left-padded to 32 bytes)
///  36..68   : tokenOut  (left-padded to 32 bytes)
///  68..100  : fee       (uint24 → uint256, big-endian)
/// 100..132  : recipient
/// 132..164  : amountIn
/// 164..196  : amountOutMinimum
/// 196..228  : sqrtPriceLimitX96
/// ```
///
/// The struct tuple is ABI-encoded inline (no offset header) per the
/// Solidity packed-struct rule — every field is a fixed-size word.
pub fn encode_exact_input_single(params: &SwapParams) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 32 * 7);
    out.extend_from_slice(&EXACT_INPUT_SINGLE_SELECTOR);
    out.extend_from_slice(&address_padded(&params.token_in));
    out.extend_from_slice(&address_padded(&params.token_out));
    out.extend_from_slice(&u32_padded(params.fee));
    out.extend_from_slice(&address_padded(&params.recipient));
    out.extend_from_slice(&params.amount_in);
    out.extend_from_slice(&params.amount_out_minimum);
    out.extend_from_slice(&params.sqrt_price_limit_x96);
    out
}

fn address_padded(addr: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(addr);
    out
}

fn u32_padded(v: u32) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[28..].copy_from_slice(&v.to_be_bytes());
    out
}

/// Hex-encode bytes as `0x...` (lowercase), suitable for JSON-RPC
/// `eth_sendRawTransaction`'s `data:` field.
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(2 + bytes.len() * 2);
    out.push_str("0x");
    for b in bytes {
        out.push(nibble_to_hex(b >> 4));
        out.push(nibble_to_hex(b & 0xF));
    }
    out
}

fn nibble_to_hex(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        10..=15 => (b'a' + (n - 10)) as char,
        _ => unreachable!(),
    }
}

/// Build the standard Sepolia Etherscan URL for a transaction hash.
/// Surface this in execution receipts so judges can verify the swap landed.
pub fn sepolia_etherscan_tx_url(tx_hash_hex: &str) -> String {
    let h = tx_hash_hex.strip_prefix("0x").unwrap_or(tx_hash_hex);
    format!("https://sepolia.etherscan.io/tx/0x{h}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiny_keccak::{Hasher, Keccak};

    #[test]
    fn selector_pinned_to_keccak() {
        // Drift-detector: if Uniswap ever changes the param tuple, the
        // selector changes, and this test fails before any live RPC call
        // does. Source: SwapRouter02.sol on the v3-periphery repo.
        let canonical =
            b"exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))";
        let mut k = Keccak::v256();
        let mut hash = [0u8; 32];
        k.update(canonical);
        k.finalize(&mut hash);
        let derived = [hash[0], hash[1], hash[2], hash[3]];
        assert_eq!(
            derived, EXACT_INPUT_SINGLE_SELECTOR,
            "selector drift detected — Uniswap struct shape changed?"
        );
    }

    #[test]
    fn parse_address_round_trip() {
        let cases = [
            "0x3bFA4769FB09eefC5a80d6E87c3B9C650f7Ae48E",
            SEPOLIA_USDC,
            SEPOLIA_WETH,
        ];
        for s in cases {
            let parsed = parse_address(s).expect("parse");
            assert_eq!(parsed.len(), 20);
        }
    }

    #[test]
    fn parse_address_rejects_short() {
        assert!(matches!(parse_address("0x123"), Err(AddressError::BadLength(3))));
    }

    #[test]
    fn parse_address_rejects_non_hex() {
        // 40-char input with 'g' at position 0 of payload → caught as non-hex.
        let bad = format!("0x{}", "g".repeat(40));
        assert!(matches!(parse_address(&bad), Err(AddressError::NonHex('g'))));
    }

    #[test]
    fn calldata_layout_total_length() {
        let params = SwapParams::sepolia_weth_for_usdc(
            [0xAA; 20],
            10_000_000_000_000_000,    // 0.01 WETH
            1_000_000,                 // 1 USDC slippage floor
        )
        .unwrap();
        let calldata = encode_exact_input_single(&params);
        assert_eq!(
            calldata.len(),
            4 + 32 * 7,
            "expected 4-byte selector + 7 × 32-byte words"
        );
        assert_eq!(&calldata[0..4], &EXACT_INPUT_SINGLE_SELECTOR);
    }

    #[test]
    fn calldata_token_in_at_word_1() {
        let recipient = [0xBB; 20];
        let params = SwapParams::sepolia_weth_for_usdc(recipient, 1, 1).unwrap();
        let calldata = encode_exact_input_single(&params);
        // Word 1 (offset 4..36): tokenIn. Last 20 bytes = WETH, first 12 = padding.
        for &b in &calldata[4..16] {
            assert_eq!(b, 0, "tokenIn word should be left-padded with zeros");
        }
        let weth_bytes = parse_address(SEPOLIA_WETH).unwrap();
        assert_eq!(&calldata[16..36], &weth_bytes);
    }

    #[test]
    fn calldata_recipient_at_word_4() {
        let recipient = [0xCC; 20];
        let params = SwapParams::sepolia_weth_for_usdc(recipient, 1, 1).unwrap();
        let calldata = encode_exact_input_single(&params);
        // Word 4 starts at offset 4 + 32*3 = 100.
        for &b in &calldata[100..112] {
            assert_eq!(b, 0);
        }
        assert_eq!(&calldata[112..132], &recipient);
    }

    #[test]
    fn calldata_amount_in_at_word_5() {
        let recipient = [0xDD; 20];
        // 0.01 WETH = 1e16 wei
        let params = SwapParams::sepolia_weth_for_usdc(recipient, 10_000_000_000_000_000, 1).unwrap();
        let calldata = encode_exact_input_single(&params);
        // Word 5 starts at offset 4 + 32*4 = 132.
        let amount_word = &calldata[132..164];
        // Last 16 bytes carry the u128; high 16 bytes are zero.
        for &b in &amount_word[..16] {
            assert_eq!(b, 0);
        }
        let amount_decoded = u128::from_be_bytes(amount_word[16..32].try_into().unwrap());
        assert_eq!(amount_decoded, 10_000_000_000_000_000);
    }

    #[test]
    fn calldata_sqrt_price_limit_zero_by_default() {
        let recipient = [0xEE; 20];
        let params = SwapParams::sepolia_weth_for_usdc(recipient, 1, 1).unwrap();
        let calldata = encode_exact_input_single(&params);
        // Word 7: offset 4 + 32*6 = 196.
        for &b in &calldata[196..228] {
            assert_eq!(b, 0, "sqrt_price_limit_x96 should be zero (no limit)");
        }
    }

    #[test]
    fn etherscan_url_strips_0x_prefix() {
        let with = sepolia_etherscan_tx_url("0xdeadbeef");
        let without = sepolia_etherscan_tx_url("deadbeef");
        assert_eq!(with, without);
        assert_eq!(with, "https://sepolia.etherscan.io/tx/0xdeadbeef");
    }

    #[test]
    fn hex_encode_smoke() {
        assert_eq!(hex_encode(&[0x12, 0xAB, 0xff]), "0x12abff");
        assert_eq!(hex_encode(&[]), "0x");
    }

    #[test]
    fn sepolia_chain_id_re_export() {
        assert_eq!(SEPOLIA_CHAIN_ID_TRADING, SEPOLIA_CHAIN_ID);
    }

    #[test]
    fn usdc_for_weth_inverts_pair() {
        let recipient = [0xFF; 20];
        let p = SwapParams::sepolia_usdc_for_weth(recipient, 1_000_000, 0).unwrap();
        let usdc = parse_address(SEPOLIA_USDC).unwrap();
        let weth = parse_address(SEPOLIA_WETH).unwrap();
        assert_eq!(p.token_in, usdc);
        assert_eq!(p.token_out, weth);
    }
}
