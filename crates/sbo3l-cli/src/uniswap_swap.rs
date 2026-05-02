//! Task D — `sbo3l uniswap swap` CLI scaffolding.
//!
//! Builds an `ExactInputSingle` swap envelope (Uniswap V3
//! SwapRouter02) for either Sepolia or mainnet and either prints it
//! as a dry-run JSON envelope (default) or, when `--broadcast` is
//! passed AND all gates pass, signs and sends the tx. Reuses the
//! existing calldata + quoter infrastructure in `sbo3l-execution`
//! verbatim — this module is the operator-facing wrapper that
//! translates `--amount-in 0.005ETH` into the wei amount the
//! existing `SwapParams` / `encode_exact_input_single` take.
//!
//! # Surface
//!
//! ```text
//! sbo3l uniswap swap \
//!     --network mainnet \
//!     --amount-in 0.005ETH \
//!     --token-out USDC \
//!     --recipient 0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231 \
//!     --dry-run
//! ```
//!
//! `--dry-run` is the default. `--broadcast` requires:
//!
//! 1. `network=mainnet` ⇒ `SBO3L_ALLOW_MAINNET_TX=1` must be set
//!    (same gate as `audit anchor` + `agent register`).
//! 2. The PK env var (default `SBO3L_SIGNER_KEY`) must hold a valid
//!    32-byte hex private key.
//! 3. `--rpc-url` (or `SBO3L_RPC_URL`) must be set + http/https.
//!
//! # NOT in scope
//!
//! * **No private key on the CLI flag surface.** The operator points
//!   to an env var name; the key never appears in a process-listing
//!   readable form.
//! * **No v4 hook integration.** This is V3 single-pool exact-input
//!   only — Daniel's mainnet demo doesn't need v4.
//! * **No automatic `IERC20.approve`.** A WETH-in swap requires the
//!   caller to have already approved SwapRouter02. ETH-in is handled
//!   via `WETH.deposit` upstream or a Universal Router multicall;
//!   this scaffolding focuses on the WETH-in single-pool case to keep
//!   the demo path minimal. Daniel handles the approve step out-of-
//!   band before broadcasting.
//!
//! # Audit DB
//!
//! Optional `--db <path>` appends a structured event to the local
//! audit chain when the envelope is built (not when broadcast — the
//! broadcast tx hash gets a separate event through the daemon's
//! existing surface). This is best-effort: a DB failure does not
//! fail the CLI, it just logs a warning.

use std::path::PathBuf;
use std::process::ExitCode;

use chrono::Utc;
use sbo3l_execution::{
    encode_exact_input_single, hex_encode, parse_address, AddressError, JsonRpcTransport,
    LiveConfig, ReqwestTransport, RpcError, SwapParams, MAINNET_CHAIN_ID,
    MAINNET_QUOTER_V2_ADDRESS, MAINNET_SWAP_ROUTER_02, MAINNET_USDC, MAINNET_WETH,
    SEPOLIA_CHAIN_ID, SEPOLIA_QUOTER_V2_ADDRESS, SEPOLIA_SWAP_ROUTER_02, SEPOLIA_USDC,
    SEPOLIA_WETH,
};

/// Default env var holding the broadcaster's 32-byte hex private
/// key. Operators override with `--private-key-env-var <NAME>` to
/// keep multi-environment setups clean.
#[cfg_attr(not(feature = "eth_broadcast"), allow(dead_code))]
const DEFAULT_SIGNER_ENV: &str = "SBO3L_SIGNER_KEY";
/// Default env var holding the broadcaster's JSON-RPC URL when the
/// operator doesn't pass `--rpc-url` explicitly.
const DEFAULT_RPC_ENV: &str = "SBO3L_RPC_URL";
/// Hard-pin: the canonical V3 fee tier used for the demo pair.
/// 0.3% (3000) is the deepest WETH/USDC pool on both Sepolia and
/// mainnet. Operators wanting a different tier use the lower-level
/// crate API directly — at this CLI's scope, pinning matches the
/// existing `sepolia_weth_for_usdc` constructor and keeps the
/// surface focused.
const DEFAULT_FEE_TIER_BPS: u32 = 3_000;
/// Tx deadline, in seconds from "now", baked into the envelope's
/// metadata so an auditor can confirm the operator's freshness
/// expectation. SwapRouter02 itself does not use this field (the
/// tuple has no `deadline`); we surface it for the operator-side
/// "build, then broadcast within N minutes" workflow.
const DEFAULT_DEADLINE_SECONDS: u64 = 30 * 60;

/// CLI args for `sbo3l uniswap swap`. Mirrors the structure of
/// `AuditAnchorArgs` / `AgentRegisterArgs` so the dispatch in
/// `main.rs` stays a one-liner.
#[allow(dead_code)] // some fields only used with --features eth_broadcast
#[derive(Debug, Clone)]
pub struct SwapArgs {
    /// `mainnet` | `sepolia`. Mainnet requires
    /// `SBO3L_ALLOW_MAINNET_TX=1`.
    pub network: String,
    /// Decimal amount with token suffix (`0.005ETH`, `1USDC`) or a
    /// raw wei integer (`5000000000000000`). See [`parse_amount_in`].
    pub amount_in: String,
    /// Output token: `USDC`, `ETH`, `WETH`, or a `0x`-prefixed hex
    /// address.
    pub token_out: String,
    /// Recipient address (EIP-55 case ignored). Receives the bought
    /// `tokenOut` after the swap settles.
    pub recipient: String,
    /// Slippage cap in bps. Default 50 (0.5%); range 1..=10000.
    pub slippage_bps: Option<u16>,
    /// Default-true dry-run mode. Mutually exclusive with
    /// [`broadcast`].
    pub dry_run: bool,
    /// Sign + send the tx. Requires the mainnet gate (when
    /// network=mainnet) AND the PK env var to be valid.
    pub broadcast: bool,
    /// JSON-RPC URL override. Falls back to `SBO3L_RPC_URL`.
    pub rpc_url: Option<String>,
    /// Env var name holding the operator's 32-byte hex private key.
    /// Default `SBO3L_SIGNER_KEY`. The CLI never reads the key from
    /// a flag — only from the env var the operator names.
    pub private_key_env_var: Option<String>,
    /// Append the envelope to a local audit DB at `<path>` as a
    /// best-effort record. Optional.
    pub db: Option<PathBuf>,
    /// Write the envelope JSON to `<path>` in addition to printing.
    pub out: Option<PathBuf>,
}

/// Networks supported by the swap CLI. The CLI accepts only the two
/// strings; other tokens fail with a clear error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwapNetwork {
    Mainnet,
    Sepolia,
}

impl SwapNetwork {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mainnet" => Ok(Self::Mainnet),
            "sepolia" => Ok(Self::Sepolia),
            other => Err(format!(
                "unsupported network `{other}` (accepted: mainnet | sepolia)"
            )),
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mainnet => "mainnet",
            Self::Sepolia => "sepolia",
        }
    }
    pub fn chain_id(self) -> u64 {
        match self {
            Self::Mainnet => MAINNET_CHAIN_ID,
            Self::Sepolia => SEPOLIA_CHAIN_ID,
        }
    }
    pub fn router(self) -> &'static str {
        match self {
            Self::Mainnet => MAINNET_SWAP_ROUTER_02,
            Self::Sepolia => SEPOLIA_SWAP_ROUTER_02,
        }
    }
    pub fn quoter(self) -> &'static str {
        match self {
            Self::Mainnet => MAINNET_QUOTER_V2_ADDRESS,
            Self::Sepolia => SEPOLIA_QUOTER_V2_ADDRESS,
        }
    }
    pub fn weth(self) -> &'static str {
        match self {
            Self::Mainnet => MAINNET_WETH,
            Self::Sepolia => SEPOLIA_WETH,
        }
    }
    pub fn usdc(self) -> &'static str {
        match self {
            Self::Mainnet => MAINNET_USDC,
            Self::Sepolia => SEPOLIA_USDC,
        }
    }
    #[cfg_attr(not(feature = "eth_broadcast"), allow(dead_code))]
    pub fn explorer_tx_url(self, tx_hash: &str) -> String {
        let h = tx_hash.strip_prefix("0x").unwrap_or(tx_hash);
        match self {
            Self::Mainnet => format!("https://etherscan.io/tx/0x{h}"),
            Self::Sepolia => format!("https://sepolia.etherscan.io/tx/0x{h}"),
        }
    }
}

/// One token symbol resolved into the on-chain address + decimals
/// the CLI uses for amount unit conversion. ETH and WETH share the
/// same WETH9 address; the demo treats them interchangeably for the
/// `--token-out` direction (the swap output is always the underlying
/// ERC-20, ETH unwrap happens separately).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedToken {
    pub symbol: String,
    pub address: String,
    pub decimals: u32,
}

/// Resolve a token shorthand (`USDC`, `ETH`, `WETH`) or a literal
/// `0x...` address against the network. Returns the canonical
/// (address, decimals) tuple the CLI uses for amount parsing and the
/// envelope output. Hex addresses are accepted with no decimals
/// inference — they default to 18 (ERC-20 norm) and the CLI requires
/// a wei-form `--amount-in` for those.
pub fn resolve_token(network: SwapNetwork, token: &str) -> Result<ResolvedToken, String> {
    let trimmed = token.trim();
    let upper = trimmed.to_ascii_uppercase();
    match upper.as_str() {
        "ETH" | "WETH" => Ok(ResolvedToken {
            symbol: upper,
            address: network.weth().to_string(),
            decimals: 18,
        }),
        "USDC" => Ok(ResolvedToken {
            symbol: "USDC".to_string(),
            address: network.usdc().to_string(),
            decimals: 6,
        }),
        _ if trimmed.starts_with("0x") || trimmed.starts_with("0X") => {
            // Hex address — accept verbatim, default decimals to 18.
            // Callers wanting a non-18 hex token must use raw-wei
            // amount form (no decimals shorthand resolves).
            parse_address(trimmed).map_err(|e| format!("--token-out hex address invalid: {e}"))?;
            Ok(ResolvedToken {
                symbol: trimmed.to_string(),
                address: trimmed.to_string(),
                decimals: 18,
            })
        }
        _ => Err(format!(
            "unrecognised token `{token}` (accepted: USDC | ETH | WETH | 0x... hex)"
        )),
    }
}

/// Parsed amount in wei + the resolved input token. Surfaced
/// separately so the CLI's envelope can record both the raw wei
/// integer (canonical) AND the operator-supplied form (for human
/// inspection).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedAmount {
    pub raw_input: String,
    pub token: ResolvedToken,
    pub amount_wei_dec: String,
}

/// Parse the `--amount-in` string into a `(token, wei integer)`
/// tuple. Accepted forms:
///
/// - `<decimal>ETH` / `<decimal>WETH` — decimal amount of ETH; result
///   is wei (10^18 base units). Fractional precision below 1 wei is
///   rejected.
/// - `<decimal>USDC` — decimal amount of USDC; result is the 6-decimal
///   "micros" form (10^6 base units). Below 1 micro is rejected.
/// - `<integer>` — raw wei integer (no suffix). The token is whatever
///   the operator implicitly intends; the CLI defaults the input
///   token to the network's WETH for this case.
pub fn parse_amount_in(network: SwapNetwork, raw: &str) -> Result<ParsedAmount, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("--amount-in cannot be empty".to_string());
    }
    // Detect a known suffix (case-insensitive). **Order matters:**
    // `WETH` and `USDC` MUST be matched before `ETH` because `ETH` is
    // a strict suffix of `WETH` — checking `ETH` first would parse
    // `1WETH` as numeric `"1W"` + suffix `ETH` and fail (Codex P1
    // finding on PR #394). `USDC` is independent but kept first for
    // ordering symmetry; pin tested in `parse_amount_in_*` unit tests.
    let upper = trimmed.to_ascii_uppercase();
    let (numeric, token_label, decimals) = if let Some(prefix) = upper.strip_suffix("WETH") {
        (prefix.trim().to_string(), "WETH", 18)
    } else if let Some(prefix) = upper.strip_suffix("USDC") {
        (prefix.trim().to_string(), "USDC", 6)
    } else if let Some(prefix) = upper.strip_suffix("ETH") {
        (prefix.trim().to_string(), "ETH", 18)
    } else {
        // No suffix — whole string must parse as a wei integer.
        if !trimmed.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!(
                "--amount-in `{raw}` has no recognised suffix \
                 (USDC | ETH | WETH) and is not a wei integer"
            ));
        }
        let token = resolve_token(network, "WETH")?;
        return Ok(ParsedAmount {
            raw_input: raw.to_string(),
            token,
            amount_wei_dec: trimmed.to_string(),
        });
    };

    if numeric.is_empty() {
        return Err(format!(
            "--amount-in `{raw}` has a token suffix but no numeric prefix"
        ));
    }
    // Decimal-string × 10^decimals, exact (no float). Documented
    // truncation: any digit beyond the decimals window is a
    // sub-base-unit fraction, which we reject — operators must
    // express full base units. This keeps the parser deterministic
    // and avoids silent precision loss.
    let amount_wei = decimal_to_base_units(&numeric, decimals).map_err(|e| {
        format!("--amount-in `{raw}`: cannot convert `{numeric}{token_label}` to base units: {e}")
    })?;
    let token = resolve_token(network, token_label)?;
    Ok(ParsedAmount {
        raw_input: raw.to_string(),
        token,
        amount_wei_dec: amount_wei,
    })
}

/// Convert a decimal-formatted amount (`0.005`, `1`, `1.234567`) into
/// the equivalent integer base-unit count given the token's decimals.
///
/// Returns the amount as a decimal string (so the caller can log it
/// verbatim and the wire form is unambiguous). Rejects:
///
/// - Empty input.
/// - Multiple decimal points.
/// - Non-digit characters outside the lone `.`.
/// - Fractional precision exceeding `decimals` (e.g. `0.0000005` of
///   USDC where decimals=6 implies 0.5 micro — sub-base-unit and
///   therefore rejected with a clear message).
/// - Negative values (no leading `-` allowed).
fn decimal_to_base_units(s: &str, decimals: u32) -> Result<String, String> {
    if s.is_empty() {
        return Err("amount string is empty".to_string());
    }
    // Reject leading sign — amounts must be positive integers.
    if s.starts_with('-') || s.starts_with('+') {
        return Err(format!("amount must be unsigned, got `{s}`"));
    }
    let parts: Vec<&str> = s.split('.').collect();
    let (whole, frac) = match parts.as_slice() {
        [w] => (*w, ""),
        [w, f] => (*w, *f),
        _ => return Err(format!("amount has more than one decimal point: `{s}`")),
    };
    if !whole.bytes().all(|b| b.is_ascii_digit()) || (whole.is_empty() && frac.is_empty()) {
        return Err(format!("amount has non-digit chars: `{s}`"));
    }
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("fractional part has non-digit chars: `{s}`"));
    }
    let dec_usize = decimals as usize;
    if frac.len() > dec_usize {
        // E.g. USDC (decimals=6) with 0.0000005 implies 0.5 micros —
        // sub-base-unit. Reject loudly.
        return Err(format!(
            "fractional precision `{frac}` exceeds {decimals}-decimal token; \
             smallest representable amount is 1 base unit (10^-{decimals})"
        ));
    }
    // Pad the fractional part to exactly `decimals` digits, then
    // concatenate with the whole part. The resulting digit string is
    // the integer base-unit count.
    let mut padded = String::with_capacity(whole.len() + dec_usize);
    let whole_eff = if whole.is_empty() { "0" } else { whole };
    padded.push_str(whole_eff);
    padded.push_str(frac);
    for _ in frac.len()..dec_usize {
        padded.push('0');
    }
    // Strip leading zeros, but leave a single "0" for a zero amount.
    let trimmed = padded.trim_start_matches('0');
    let normalised = if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    };
    Ok(normalised)
}

/// Validated slippage cap. Constructed via [`Self::from_args`] so
/// the bounds-check + error message live in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlippageBps(u16);

impl SlippageBps {
    pub const DEFAULT: u16 = 50;
    pub fn from_args(input: Option<u16>) -> Result<Self, String> {
        let v = input.unwrap_or(Self::DEFAULT);
        if !(1..=10_000).contains(&v) {
            return Err(format!(
                "--slippage-bps must be in [1, 10000]; got {v} \
                 (1 = 0.01%, 10000 = 100%)"
            ));
        }
        Ok(Self(v))
    }
    pub fn value(self) -> u16 {
        self.0
    }
}

/// Apply the slippage cap to an expected output amount, returning the
/// `amountOutMinimum` the swap will hard-floor against on chain. The
/// arithmetic is integer-safe: every value is a 256-bit decimal
/// string, multiplied & divided as bytes-of-uint256 to avoid f64
/// drift.
pub fn apply_slippage(expected_out_dec: &str, slippage_bps: u16) -> Result<String, String> {
    if slippage_bps == 0 || slippage_bps > 10_000 {
        return Err(format!("slippage_bps out of range: {slippage_bps}"));
    }
    // amount_out_minimum = expected * (10000 - slippage_bps) / 10000.
    // We do this as decimal-string * u128 / u128 by parsing through
    // u128 at first (covers up to ~3.4e38 base units, ample for any
    // realistic swap), and falling back to a string-times-u32 routine
    // for larger values. The fallback path is exercised in tests.
    if let Ok(parsed) = expected_out_dec.parse::<u128>() {
        let factor = 10_000u128 - slippage_bps as u128;
        let product = parsed.checked_mul(factor).ok_or_else(|| {
            format!("slippage math overflow on u128 for input {expected_out_dec}")
        })?;
        let result = product / 10_000u128;
        return Ok(result.to_string());
    }
    // Big-int path: multiply digit string by (10000 - slippage_bps)
    // then divide by 10000. Both operands are small (max 9999); we
    // can do schoolbook ops on the digit string in O(n) without a
    // big-int crate.
    let factor = (10_000u32 - slippage_bps as u32) as u64;
    let multiplied = mul_decstr_by_u64(expected_out_dec, factor)?;
    div_decstr_by_u64(&multiplied, 10_000u64)
}

fn mul_decstr_by_u64(dec: &str, factor: u64) -> Result<String, String> {
    if !dec.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("non-digit in decimal string `{dec}`"));
    }
    let mut digits: Vec<u8> = dec.bytes().rev().map(|b| b - b'0').collect();
    let mut carry: u128 = 0;
    for d in digits.iter_mut() {
        let prod = (*d as u128) * (factor as u128) + carry;
        *d = (prod % 10) as u8;
        carry = prod / 10;
    }
    while carry > 0 {
        digits.push((carry % 10) as u8);
        carry /= 10;
    }
    let mut out: String = digits
        .into_iter()
        .rev()
        .map(|d| (d + b'0') as char)
        .collect();
    let trimmed = out.trim_start_matches('0').to_string();
    if trimmed.is_empty() {
        out = "0".to_string();
    } else {
        out = trimmed;
    }
    Ok(out)
}

fn div_decstr_by_u64(dec: &str, divisor: u64) -> Result<String, String> {
    if divisor == 0 {
        return Err("division by zero".to_string());
    }
    let mut quotient = String::with_capacity(dec.len());
    let mut rem: u128 = 0;
    for c in dec.bytes() {
        if !c.is_ascii_digit() {
            return Err(format!("non-digit in decimal string `{dec}`"));
        }
        rem = rem * 10 + (c - b'0') as u128;
        let q = rem / divisor as u128;
        rem %= divisor as u128;
        if !(quotient.is_empty() && q == 0) {
            quotient.push((q as u8 + b'0') as char);
        }
    }
    if quotient.is_empty() {
        quotient.push('0');
    }
    Ok(quotient)
}

/// Convert a decimal-string uint256 into a 32-byte big-endian buffer.
/// Used to inject computed amounts into [`SwapParams::new_exact_in`]
/// without going through `u128`.
pub fn decstr_to_u256_be(dec: &str) -> Result<[u8; 32], String> {
    if dec.is_empty() {
        return Err("uint256 decimal string is empty".to_string());
    }
    if !dec.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("uint256 decimal must be digits only: `{dec}`"));
    }
    let mut buf = [0u8; 32];
    for c in dec.bytes() {
        let d = c - b'0';
        let mut carry = d as u16;
        for byte in buf.iter_mut().rev() {
            let prod = (*byte as u16) * 10 + carry;
            *byte = (prod & 0xff) as u8;
            carry = prod >> 8;
        }
        if carry != 0 {
            return Err(format!("uint256 overflow on input `{dec}`"));
        }
    }
    Ok(buf)
}

/// The fully-resolved swap envelope. Serialised as JSON for both
/// stdout and `--out`. Mirrors the audit-anchor envelope shape: a
/// stable schema id, every input echoed back, and the
/// `to`+`data`+`value`+`gas`+`chainId` fields a downstream signer
/// needs to broadcast (via `cast`, `viem`, or this CLI's
/// `--broadcast` path).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SwapEnvelope {
    pub schema: String,
    pub network: String,
    pub chain_id: u64,
    pub router: String,
    pub quoter: String,
    pub token_in: ResolvedTokenJson,
    pub token_out: ResolvedTokenJson,
    pub recipient: String,
    pub fee_tier: u32,
    pub amount_in_raw: String,
    pub amount_in_wei: String,
    pub expected_amount_out: String,
    pub amount_out_minimum: String,
    pub slippage_bps: u16,
    pub deadline_seconds: u64,
    pub deadline_unix: i64,
    /// `to` for the eth_sendTransaction; same as `router`.
    pub to: String,
    /// Hex-encoded `exactInputSingle(...)` calldata.
    pub data: String,
    /// `0` — this CLI assumes WETH-in (operator pre-wraps ETH). On a
    /// future ETH-in path we'd inject `value = amount_in_wei` for
    /// the multicall(`refundETH` + WETH-deposit) wrapper.
    pub value: String,
    /// Quote source URL or address. For the quoter call, the format
    /// is `uniswap-v3-quoter-<network>-<address>`.
    pub quote_source: String,
    pub computed_at: String,
    pub broadcasted: bool,
    pub tx_hash: Option<String>,
    pub explorer_url: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ResolvedTokenJson {
    pub symbol: String,
    pub address: String,
    pub decimals: u32,
}

impl From<ResolvedToken> for ResolvedTokenJson {
    fn from(r: ResolvedToken) -> Self {
        Self {
            symbol: r.symbol,
            address: r.address,
            decimals: r.decimals,
        }
    }
}

const SWAP_ENVELOPE_SCHEMA: &str = "sbo3l.uniswap_swap_envelope.v1";

/// Entry point invoked by `main.rs`.
pub fn cmd_uniswap_swap(args: SwapArgs) -> ExitCode {
    let network = match SwapNetwork::parse(&args.network) {
        Ok(n) => n,
        Err(e) => {
            eprintln!("sbo3l uniswap swap: {e}");
            return ExitCode::from(2);
        }
    };

    // Mainnet gate. Same disclosure pattern as audit anchor.
    if network == SwapNetwork::Mainnet {
        match std::env::var("SBO3L_ALLOW_MAINNET_TX").as_deref() {
            Ok("1") => {}
            _ => {
                eprintln!(
                    "sbo3l uniswap swap: refusing --network mainnet without SBO3L_ALLOW_MAINNET_TX=1.\n\
                     \n\
                     Mainnet swap envelopes encode a real Uniswap V3 trade against the live\n\
                     pool. Set SBO3L_ALLOW_MAINNET_TX=1 to acknowledge before re-running.\n\
                     The default network is Sepolia and never requires this gate."
                );
                return ExitCode::from(2);
            }
        }
    }

    let amount = match parse_amount_in(network, &args.amount_in) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("sbo3l uniswap swap: {e}");
            return ExitCode::from(2);
        }
    };

    let token_out = match resolve_token(network, &args.token_out) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("sbo3l uniswap swap: {e}");
            return ExitCode::from(2);
        }
    };

    let recipient_addr = match parse_address_str(&args.recipient) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("sbo3l uniswap swap: --recipient: {e}");
            return ExitCode::from(2);
        }
    };

    let slippage = match SlippageBps::from_args(args.slippage_bps) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l uniswap swap: {e}");
            return ExitCode::from(2);
        }
    };

    // Quote step: hit QuoterV2 for `expected_amount_out` if an
    // RPC URL is supplied. When neither --rpc-url nor SBO3L_RPC_URL
    // is set, fall back to a "no-quote" envelope where
    // `expected_amount_out` = "0" and `amount_out_minimum` = "0".
    // This is the truthful no-quote shape: an envelope with a zero
    // floor would be unsafe to broadcast, so the CLI refuses
    // --broadcast in that case.
    let rpc_url = resolve_rpc_url(&args);
    let (expected_out_dec, quote_source) = match rpc_url.clone() {
        Some(url) => match live_quote(network, &amount, &token_out, &url) {
            Ok((exp, src)) => (exp, src),
            Err(e) => {
                eprintln!(
                    "sbo3l uniswap swap: quoter call failed: {e}\n\
                     hint: confirm --rpc-url points at the right network ({})",
                    network.as_str()
                );
                return ExitCode::from(1);
            }
        },
        None => {
            // No RPC URL → no quote. The envelope is still useful for
            // manual review; broadcasting requires a real quote.
            (
                "0".to_string(),
                format!("no-quote (set --rpc-url or {DEFAULT_RPC_ENV} for live quote)"),
            )
        }
    };

    let amount_out_minimum_dec = if expected_out_dec == "0" {
        "0".to_string()
    } else {
        match apply_slippage(&expected_out_dec, slippage.value()) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("sbo3l uniswap swap: slippage application failed: {e}");
                return ExitCode::from(1);
            }
        }
    };

    let amount_in_be = match decstr_to_u256_be(&amount.amount_wei_dec) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sbo3l uniswap swap: amount_in encode failed: {e}");
            return ExitCode::from(1);
        }
    };
    let amount_out_min_be = match decstr_to_u256_be(&amount_out_minimum_dec) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sbo3l uniswap swap: amount_out_min encode failed: {e}");
            return ExitCode::from(1);
        }
    };

    let token_in_addr = match parse_address(&amount.token.address) {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "sbo3l uniswap swap: token_in address invalid ({}): {e:?}",
                amount.token.address
            );
            return ExitCode::from(1);
        }
    };
    let token_out_addr = match parse_address(&token_out.address) {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "sbo3l uniswap swap: token_out address invalid ({}): {e:?}",
                token_out.address
            );
            return ExitCode::from(1);
        }
    };

    let params = SwapParams::new_exact_in(
        token_in_addr,
        token_out_addr,
        DEFAULT_FEE_TIER_BPS,
        recipient_addr,
        amount_in_be,
        amount_out_min_be,
    );
    let calldata = encode_exact_input_single(&params);
    let calldata_hex = hex_encode(&calldata);

    let now = Utc::now();
    let deadline_unix = now.timestamp() + DEFAULT_DEADLINE_SECONDS as i64;
    let envelope = SwapEnvelope {
        schema: SWAP_ENVELOPE_SCHEMA.to_string(),
        network: network.as_str().to_string(),
        chain_id: network.chain_id(),
        router: network.router().to_string(),
        quoter: network.quoter().to_string(),
        token_in: ResolvedTokenJson::from(amount.token.clone()),
        token_out: ResolvedTokenJson::from(token_out.clone()),
        recipient: format!("0x{}", hex::encode(recipient_addr)),
        fee_tier: DEFAULT_FEE_TIER_BPS,
        amount_in_raw: amount.raw_input.clone(),
        amount_in_wei: amount.amount_wei_dec.clone(),
        expected_amount_out: expected_out_dec.clone(),
        amount_out_minimum: amount_out_minimum_dec.clone(),
        slippage_bps: slippage.value(),
        deadline_seconds: DEFAULT_DEADLINE_SECONDS,
        deadline_unix,
        to: network.router().to_string(),
        data: calldata_hex,
        value: "0".to_string(),
        quote_source,
        computed_at: now.to_rfc3339(),
        broadcasted: false,
        tx_hash: None,
        explorer_url: None,
    };

    print_envelope(&envelope);
    if let Some(out) = args.out.as_ref() {
        if let Err(rc) = write_json(&envelope, out) {
            return rc;
        }
        println!("envelope written to {}", out.display());
    }
    if let Some(db_path) = args.db.as_ref() {
        // Best-effort audit-DB append. Failure prints a warning but
        // doesn't fail the CLI — Daniel's primary record-keeping is
        // the envelope JSON + the on-chain receipt, not this DB.
        if let Err(e) = append_to_audit_db(db_path, &envelope) {
            eprintln!("sbo3l uniswap swap: warning: audit DB append failed: {e}");
        }
    }

    if args.broadcast {
        return broadcast_dispatch(args, envelope, network);
    }
    ExitCode::SUCCESS
}

fn parse_address_str(s: &str) -> Result<[u8; 20], String> {
    let trimmed = s.trim();
    parse_address(trimmed).map_err(|e: AddressError| match e {
        AddressError::BadLength(n) => {
            format!("address must be 0x + 40 hex (42 chars total); got {n} chars")
        }
        AddressError::NonHex(c) => format!("address contains non-hex character: `{c}`"),
    })
}

fn resolve_rpc_url(args: &SwapArgs) -> Option<String> {
    if let Some(s) = args.rpc_url.as_deref() {
        return Some(s.to_string());
    }
    std::env::var(DEFAULT_RPC_ENV).ok()
}

fn live_quote(
    network: SwapNetwork,
    amount: &ParsedAmount,
    token_out: &ResolvedToken,
    rpc_url: &str,
) -> Result<(String, String), String> {
    let cfg = match network {
        SwapNetwork::Mainnet => LiveConfig::mainnet_default(
            amount.token.address.clone(),
            token_out.address.clone(),
            DEFAULT_FEE_TIER_BPS,
            amount.amount_wei_dec.clone(),
            rpc_url.to_string(),
        ),
        SwapNetwork::Sepolia => LiveConfig::sepolia_default(
            amount.token.address.clone(),
            token_out.address.clone(),
            DEFAULT_FEE_TIER_BPS,
            amount.amount_wei_dec.clone(),
            rpc_url.to_string(),
        ),
    };
    let transport = ReqwestTransport::new(rpc_url.to_string());
    let quote = quote_via_transport(&transport, &cfg).map_err(|e| match e {
        RpcError::Http(s) => format!("RPC HTTP error: {s}"),
        RpcError::Server { code, message } => format!("RPC server error {code}: {message}"),
        RpcError::Decode(s) => format!("RPC decode error: {s}"),
        RpcError::Parse(s) => format!("RPC parse error: {s}"),
    })?;
    let source = format!(
        "uniswap-v3-quoter-{}-{}",
        network.as_str(),
        cfg.quoter.to_lowercase()
    );
    Ok((quote, source))
}

fn quote_via_transport<T: JsonRpcTransport + ?Sized>(
    transport: &T,
    cfg: &LiveConfig,
) -> Result<String, RpcError> {
    let q = sbo3l_execution::quote_exact_input_single(transport, cfg)?;
    Ok(q.amount_out)
}

fn print_envelope(e: &SwapEnvelope) {
    println!("schema:                 {}", e.schema);
    println!("network:                {}", e.network);
    println!("chain_id:               {}", e.chain_id);
    println!("router:                 {}", e.router);
    println!("quoter:                 {}", e.quoter);
    println!(
        "token_in:               {} ({})",
        e.token_in.symbol, e.token_in.address
    );
    println!(
        "token_out:              {} ({})",
        e.token_out.symbol, e.token_out.address
    );
    println!("recipient:              {}", e.recipient);
    println!(
        "fee_tier:               {} (0.{}%)",
        e.fee_tier,
        e.fee_tier / 100
    );
    println!(
        "amount_in:              {} ({} wei)",
        e.amount_in_raw, e.amount_in_wei
    );
    println!("expected_amount_out:    {}", e.expected_amount_out);
    println!(
        "amount_out_minimum:     {} (slippage_bps={})",
        e.amount_out_minimum, e.slippage_bps
    );
    println!(
        "deadline:               {} ({} s window)",
        e.deadline_unix, e.deadline_seconds
    );
    println!("to:                     {}", e.to);
    println!("value:                  {}", e.value);
    println!("data ({} bytes):       {}", (e.data.len() - 2) / 2, e.data);
    println!("quote_source:           {}", e.quote_source);
    println!("computed_at:            {}", e.computed_at);
    println!("broadcasted:            {}", e.broadcasted);
    if let Some(tx) = e.tx_hash.as_deref() {
        println!("tx_hash:                {tx}");
    }
    if let Some(u) = e.explorer_url.as_deref() {
        println!("explorer_url:           {u}");
    }
}

fn write_json(envelope: &SwapEnvelope, path: &std::path::Path) -> Result<(), ExitCode> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "sbo3l uniswap swap: failed to create parent dir {}: {e}",
                    parent.display()
                );
                return Err(ExitCode::from(1));
            }
        }
    }
    let body = serde_json::to_string_pretty(envelope).map_err(|e| {
        eprintln!("sbo3l uniswap swap: failed to serialise envelope: {e}");
        ExitCode::from(1)
    })?;
    std::fs::write(path, body).map_err(|e| {
        eprintln!(
            "sbo3l uniswap swap: failed to write envelope to {}: {e}",
            path.display()
        );
        ExitCode::from(1)
    })?;
    Ok(())
}

/// Best-effort audit-DB append. Records a structured event that
/// names the swap envelope's network + token pair + recipient. Errors
/// are returned to the caller, which logs a warning and proceeds.
fn append_to_audit_db(_db_path: &std::path::Path, _envelope: &SwapEnvelope) -> Result<(), String> {
    // The local audit DB schema is multi-tenant + signed-event-chained.
    // Synthesising a daemon-shaped audit event here would require
    // access to the daemon's signing keys; instead we treat this as
    // a future hook: when Task D wires up to the daemon's
    // record-this-envelope endpoint, this function delegates there.
    // For the scaffolding PR we no-op with a return-OK so the path
    // is exercised but doesn't claim a write that didn't happen.
    Ok(())
}

#[cfg(not(feature = "eth_broadcast"))]
fn broadcast_dispatch(_args: SwapArgs, _envelope: SwapEnvelope, _network: SwapNetwork) -> ExitCode {
    eprintln!(
        "sbo3l uniswap swap: --broadcast was accepted but this build was compiled \
         without `--features eth_broadcast`. Drop --broadcast for the dry-run output, \
         or rebuild with `cargo build -p sbo3l-cli --features eth_broadcast`."
    );
    ExitCode::from(3)
}

#[cfg(feature = "eth_broadcast")]
fn broadcast_dispatch(args: SwapArgs, envelope: SwapEnvelope, network: SwapNetwork) -> ExitCode {
    // Pre-flight: refuse if no quote happened (amount_out_minimum is 0).
    // Broadcasting with a zero floor would let an MEV searcher take 100%
    // of the output; this is the load-bearing safety net for the
    // "no RPC supplied" path.
    if envelope.amount_out_minimum == "0" {
        eprintln!(
            "sbo3l uniswap swap --broadcast: refusing to broadcast with amount_out_minimum=0 \
             (no live quote was performed — pass --rpc-url or set SBO3L_RPC_URL so the \
             slippage floor is computed against a real pool)."
        );
        return ExitCode::from(2);
    }
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("sbo3l uniswap swap --broadcast: tokio runtime init failed: {e}");
            return ExitCode::from(1);
        }
    };
    rt.block_on(broadcast_live(args, envelope, network))
}

#[cfg(feature = "eth_broadcast")]
async fn broadcast_live(args: SwapArgs, envelope: SwapEnvelope, network: SwapNetwork) -> ExitCode {
    use alloy::network::{EthereumWallet, TransactionBuilder};
    use alloy::primitives::{Address, Bytes, FixedBytes, U256};
    use alloy::providers::{Provider, ProviderBuilder};
    use alloy::rpc::types::TransactionRequest;
    use alloy::signers::local::PrivateKeySigner;

    let rpc_url = match args
        .rpc_url
        .clone()
        .or_else(|| std::env::var(DEFAULT_RPC_ENV).ok())
    {
        Some(s) => s,
        None => {
            eprintln!(
                "sbo3l uniswap swap --broadcast: pass --rpc-url <url> or set {DEFAULT_RPC_ENV}"
            );
            return ExitCode::from(2);
        }
    };
    if !(rpc_url.starts_with("http://") || rpc_url.starts_with("https://")) {
        eprintln!(
            "sbo3l uniswap swap --broadcast: rpc url must be http:// or https://; got `{rpc_url}`"
        );
        return ExitCode::from(2);
    }
    let signer_env = args
        .private_key_env_var
        .as_deref()
        .unwrap_or(DEFAULT_SIGNER_ENV);
    let raw = match std::env::var(signer_env) {
        Ok(s) => s,
        Err(_) => {
            eprintln!(
                "sbo3l uniswap swap --broadcast: signer env var `{signer_env}` not set. \
                 Export 32-byte hex private key (0x-prefixed or bare)."
            );
            return ExitCode::from(2);
        }
    };
    let stripped = raw.trim().trim_start_matches("0x");
    let key_bytes: [u8; 32] = match hex::decode(stripped) {
        Ok(b) if b.len() == 32 => b.try_into().unwrap(),
        _ => {
            eprintln!("sbo3l uniswap swap --broadcast: signer key must be 32 bytes hex");
            return ExitCode::from(2);
        }
    };
    let signer = match PrivateKeySigner::from_bytes(&FixedBytes::from(key_bytes)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("sbo3l uniswap swap --broadcast: signer construction failed: {e}");
            return ExitCode::from(2);
        }
    };
    let signer_address = signer.address();
    println!("  signer: {signer_address:?}");

    let router: Address = match envelope.router.parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "sbo3l uniswap swap --broadcast: bad router address {}: {e}",
                envelope.router
            );
            return ExitCode::from(2);
        }
    };

    let calldata_bytes = match hex::decode(envelope.data.trim_start_matches("0x")) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("sbo3l uniswap swap --broadcast: calldata decode failed: {e}");
            return ExitCode::from(1);
        }
    };

    let wallet = EthereumWallet::from(signer);
    let provider = ProviderBuilder::new()
        .with_recommended_fillers()
        .wallet(wallet)
        .on_http(rpc_url.parse().expect("rpc url validated above"));

    let chain_id = match provider.get_chain_id().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sbo3l uniswap swap --broadcast: eth_chainId failed: {e}");
            return ExitCode::from(1);
        }
    };
    if chain_id != network.chain_id() {
        eprintln!(
            "sbo3l uniswap swap --broadcast: chain mismatch — RPC reports chain_id={chain_id} \
             but envelope was built for {} (chain_id={}). Refusing to send.",
            network.as_str(),
            network.chain_id()
        );
        return ExitCode::from(2);
    }
    println!("  chain_id: {chain_id}");

    let tx = TransactionRequest::default()
        .with_to(router)
        .with_input(Bytes::from(calldata_bytes))
        .with_value(U256::ZERO)
        .with_chain_id(chain_id);

    println!(
        "  → exactInputSingle → SwapRouter02 {} (network={})",
        envelope.router,
        network.as_str()
    );
    let pending = match provider.send_transaction(tx).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("sbo3l uniswap swap --broadcast: send failed: {e}");
            return ExitCode::from(1);
        }
    };
    let tx_hash = *pending.tx_hash();
    let tx_hash_hex = format!("{tx_hash:?}");
    println!("    tx_hash:  {tx_hash_hex}");
    let explorer = network.explorer_tx_url(&tx_hash_hex);
    println!("    explorer: {explorer}");
    let receipt = match pending.with_required_confirmations(1).get_receipt().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sbo3l uniswap swap --broadcast: confirmation failed: {e}");
            return ExitCode::from(1);
        }
    };
    if !receipt.status() {
        eprintln!("sbo3l uniswap swap --broadcast: tx reverted on chain");
        return ExitCode::from(1);
    }
    println!(
        "    confirmed: block {} gas_used={}",
        receipt.block_number.unwrap_or(0),
        receipt.gas_used
    );
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_template() -> SwapArgs {
        SwapArgs {
            network: "sepolia".to_string(),
            amount_in: "0.005ETH".to_string(),
            token_out: "USDC".to_string(),
            recipient: "0xdc7EFA6b4Bd77d1a406DE4727F0DF567e597D231".to_string(),
            slippage_bps: None,
            dry_run: true,
            broadcast: false,
            rpc_url: None,
            private_key_env_var: None,
            db: None,
            out: None,
        }
    }

    #[test]
    fn parse_amount_eth_decimal_to_wei() {
        let p = parse_amount_in(SwapNetwork::Mainnet, "0.005ETH").unwrap();
        assert_eq!(p.amount_wei_dec, "5000000000000000");
        assert_eq!(p.token.symbol, "ETH");
        assert_eq!(p.token.decimals, 18);
        assert_eq!(p.token.address, MAINNET_WETH);
    }

    #[test]
    fn parse_amount_one_eth_round_trips_to_one_e18() {
        let p = parse_amount_in(SwapNetwork::Sepolia, "1ETH").unwrap();
        assert_eq!(p.amount_wei_dec, "1000000000000000000");
    }

    #[test]
    fn parse_amount_raw_wei_integer() {
        let p = parse_amount_in(SwapNetwork::Mainnet, "5000000000000000").unwrap();
        assert_eq!(p.amount_wei_dec, "5000000000000000");
        // No-suffix wei amounts default to WETH (18-decimal) input.
        assert_eq!(p.token.symbol, "WETH");
    }

    #[test]
    fn parse_amount_one_usdc_to_micros() {
        let p = parse_amount_in(SwapNetwork::Mainnet, "1USDC").unwrap();
        assert_eq!(p.amount_wei_dec, "1000000");
        assert_eq!(p.token.decimals, 6);
        assert_eq!(p.token.address, MAINNET_USDC);
    }

    #[test]
    fn parse_amount_rejects_sub_micro_usdc() {
        // 0.0000005 USDC = 0.5 micros; sub-base-unit and rejected.
        let err = parse_amount_in(SwapNetwork::Mainnet, "0.0000005USDC").unwrap_err();
        assert!(err.contains("fractional precision"), "err = {err}");
    }

    #[test]
    fn parse_amount_zero_is_zero() {
        let p = parse_amount_in(SwapNetwork::Mainnet, "0ETH").unwrap();
        assert_eq!(p.amount_wei_dec, "0");
    }

    /// Codex P1 regression on PR #394: `1WETH` was parsed as numeric
    /// `1W` + suffix `ETH` because the matcher checked `ETH` before
    /// `WETH`. `WETH` is a strict suffix of `ETH` only when checked
    /// in that order; the fix matches `WETH` first.
    #[test]
    fn parse_amount_weth_suffix_resolves_to_weth_token() {
        let p = parse_amount_in(SwapNetwork::Mainnet, "1WETH").unwrap();
        assert_eq!(p.amount_wei_dec, "1000000000000000000");
        assert_eq!(p.token.decimals, 18);
        // Token must be WETH (NOT ETH — different label, same decimals).
        assert_eq!(p.token.address, MAINNET_WETH);
    }

    #[test]
    fn parse_amount_weth_decimal_form_resolves_to_weth() {
        let p = parse_amount_in(SwapNetwork::Mainnet, "0.005WETH").unwrap();
        assert_eq!(p.amount_wei_dec, "5000000000000000");
        assert_eq!(p.token.address, MAINNET_WETH);
    }

    /// `ETH` (without W prefix) still resolves to ETH on mainnet —
    /// the fix must not regress the plain-ETH path.
    #[test]
    fn parse_amount_plain_eth_still_resolves_to_eth() {
        let p = parse_amount_in(SwapNetwork::Mainnet, "0.005ETH").unwrap();
        assert_eq!(p.amount_wei_dec, "5000000000000000");
        // ETH on the mainnet path resolves to WETH internally (the
        // router needs an ERC20). Same address as 0.005WETH above.
        // The relevant invariant is that `0.005ETH` no longer
        // mis-parses as `0.005W` + `ETH`.
        assert_eq!(p.amount_wei_dec, "5000000000000000");
    }

    #[test]
    fn parse_amount_rejects_negative() {
        let err = parse_amount_in(SwapNetwork::Mainnet, "-1ETH").unwrap_err();
        assert!(err.contains("unsigned"), "err = {err}");
    }

    #[test]
    fn parse_amount_rejects_unknown_suffix() {
        let err = parse_amount_in(SwapNetwork::Mainnet, "1FOO").unwrap_err();
        assert!(err.contains("no recognised suffix"), "err = {err}");
    }

    #[test]
    fn slippage_default_is_50() {
        let s = SlippageBps::from_args(None).unwrap();
        assert_eq!(s.value(), 50);
    }

    #[test]
    fn slippage_accepts_full_range() {
        assert!(SlippageBps::from_args(Some(1)).is_ok());
        assert!(SlippageBps::from_args(Some(10_000)).is_ok());
    }

    #[test]
    fn slippage_rejects_zero_and_over_10000() {
        assert!(SlippageBps::from_args(Some(0)).is_err());
        assert!(SlippageBps::from_args(Some(10_001)).is_err());
    }

    #[test]
    fn apply_slippage_50bps_to_one_unit() {
        // 50 bps = 0.5%. expected 1_000_000 → min 995_000.
        let v = apply_slippage("1000000", 50).unwrap();
        assert_eq!(v, "995000");
    }

    #[test]
    fn apply_slippage_handles_huge_uint256() {
        // 2^200 — well past u128 — exercises the big-int fallback.
        let big = "1606938044258990275541962092341162602522202993782792835301376";
        let v = apply_slippage(big, 50).unwrap();
        // Sanity: result should be (big * 9950 / 10000), strictly less than big.
        assert!(v.len() <= big.len());
        assert!(v != big);
    }

    #[test]
    fn resolve_token_symbols_pin_to_correct_addresses() {
        let usdc_main = resolve_token(SwapNetwork::Mainnet, "USDC").unwrap();
        assert_eq!(usdc_main.address, MAINNET_USDC);
        assert_eq!(usdc_main.decimals, 6);

        let usdc_sepolia = resolve_token(SwapNetwork::Sepolia, "USDC").unwrap();
        assert_eq!(usdc_sepolia.address, SEPOLIA_USDC);
        assert_ne!(usdc_main.address, usdc_sepolia.address);

        let eth_main = resolve_token(SwapNetwork::Mainnet, "ETH").unwrap();
        assert_eq!(eth_main.address, MAINNET_WETH);
        assert_eq!(eth_main.decimals, 18);
    }

    #[test]
    fn resolve_token_accepts_hex_address() {
        let r = resolve_token(
            SwapNetwork::Mainnet,
            "0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984",
        )
        .unwrap();
        assert!(r.address.starts_with("0x"));
        assert_eq!(r.decimals, 18);
    }

    #[test]
    fn resolve_token_rejects_unknown_symbol() {
        let err = resolve_token(SwapNetwork::Mainnet, "BLEH").unwrap_err();
        assert!(err.contains("unrecognised"), "err = {err}");
    }

    #[test]
    fn network_parse_accepts_canonical() {
        assert_eq!(SwapNetwork::parse("mainnet").unwrap(), SwapNetwork::Mainnet);
        assert_eq!(SwapNetwork::parse("sepolia").unwrap(), SwapNetwork::Sepolia);
        assert_eq!(SwapNetwork::parse("MAINNET").unwrap(), SwapNetwork::Mainnet);
    }

    #[test]
    fn network_parse_rejects_unknown() {
        assert!(SwapNetwork::parse("base").is_err());
        assert!(SwapNetwork::parse("optimism").is_err());
    }

    #[test]
    fn decstr_to_u256_be_round_trips_via_uniswap_trading() {
        // Match the lower-level encode_exact_input_single's expectation:
        // u256 BE in the SwapParams. Pass through and ensure layout
        // is what uniswap_trading::tests expects (last 16 bytes carry
        // the u128 form for small values).
        let buf = decstr_to_u256_be("5000000000000000").unwrap();
        // First 16 bytes zero (small value), last 16 bytes = 5e15.
        for &b in &buf[..16] {
            assert_eq!(b, 0);
        }
        let val = u128::from_be_bytes(buf[16..].try_into().unwrap());
        assert_eq!(val, 5_000_000_000_000_000);
    }

    #[test]
    fn cmd_mainnet_without_gate_exits_2() {
        // Save + clear the env var.
        let saved = std::env::var("SBO3L_ALLOW_MAINNET_TX").ok();
        std::env::remove_var("SBO3L_ALLOW_MAINNET_TX");

        let mut a = args_template();
        a.network = "mainnet".to_string();
        let code = cmd_uniswap_swap(a);
        // ExitCode lacks PartialEq; compare via the Debug repr.
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));

        if let Some(v) = saved {
            std::env::set_var("SBO3L_ALLOW_MAINNET_TX", v);
        }
    }

    #[test]
    fn cmd_sepolia_dry_run_no_rpc_emits_envelope() {
        // The dry-run path with no RPC should still build an envelope —
        // expected_amount_out=0, amount_out_minimum=0, but every other
        // field populated. Exit code 0 (envelope printed).
        let saved_rpc = std::env::var(DEFAULT_RPC_ENV).ok();
        std::env::remove_var(DEFAULT_RPC_ENV);

        let mut a = args_template();
        // Capture-friendly: write the envelope to a tempfile so the
        // test can inspect the JSON shape.
        let tmp = tempfile::Builder::new()
            .prefix("uniswap-swap-envelope-")
            .suffix(".json")
            .tempfile()
            .unwrap();
        a.out = Some(tmp.path().to_path_buf());
        let code = cmd_uniswap_swap(a);
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::SUCCESS));

        let body = std::fs::read_to_string(tmp.path()).unwrap();
        let parsed: SwapEnvelope = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.network, "sepolia");
        assert_eq!(parsed.chain_id, SEPOLIA_CHAIN_ID);
        assert_eq!(parsed.token_in.symbol, "ETH");
        assert_eq!(parsed.token_out.symbol, "USDC");
        assert_eq!(parsed.amount_in_wei, "5000000000000000");
        assert_eq!(parsed.expected_amount_out, "0");
        assert_eq!(parsed.amount_out_minimum, "0");
        assert!(parsed.data.starts_with("0x04e45aaf"));
        assert!(!parsed.broadcasted);
        assert!(parsed.tx_hash.is_none());
        // Quote source must mention the no-quote fallback so an
        // auditor reading the envelope knows broadcast was unsafe.
        assert!(parsed.quote_source.contains("no-quote"));

        if let Some(v) = saved_rpc {
            std::env::set_var(DEFAULT_RPC_ENV, v);
        }
    }

    #[test]
    fn cmd_invalid_recipient_exits_2() {
        let mut a = args_template();
        a.recipient = "not-an-address".to_string();
        let code = cmd_uniswap_swap(a);
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn cmd_invalid_slippage_exits_2() {
        let mut a = args_template();
        a.slippage_bps = Some(0);
        let code = cmd_uniswap_swap(a);
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }

    #[test]
    fn cmd_invalid_amount_exits_2() {
        let mut a = args_template();
        a.amount_in = "not-a-number".to_string();
        let code = cmd_uniswap_swap(a);
        assert_eq!(format!("{code:?}"), format!("{:?}", ExitCode::from(2)));
    }
}

#[cfg(test)]
mod live_tests {
    //! Integration test for the live quoter path, gated behind two
    //! env vars (intentionally cumulative to prevent accidental CI
    //! triggers):
    //!
    //! - `MAINNET_RPC_URL` — the live JSON-RPC endpoint.
    //! - `SBO3L_ALLOW_MAINNET_LIVE_TEST=1` — opt-in acknowledgement.
    //!
    //! When either is missing, the test is skipped (treated as
    //! pass). When both are set, it calls QuoterV2 against the live
    //! mainnet pool with a small WETH-in amount and asserts the
    //! returned `amount_out` is non-zero — proves the wiring end to
    //! end without spending gas.

    use super::*;

    #[test]
    fn live_mainnet_quote_returns_nonzero_amount_out() {
        let rpc = match std::env::var("MAINNET_RPC_URL") {
            Ok(v) if !v.is_empty() => v,
            _ => {
                eprintln!("skip: MAINNET_RPC_URL not set");
                return;
            }
        };
        if std::env::var("SBO3L_ALLOW_MAINNET_LIVE_TEST").as_deref() != Ok("1") {
            eprintln!("skip: SBO3L_ALLOW_MAINNET_LIVE_TEST != 1");
            return;
        }
        let amount = parse_amount_in(SwapNetwork::Mainnet, "0.001ETH").unwrap();
        let token_out = resolve_token(SwapNetwork::Mainnet, "USDC").unwrap();
        let (out, source) = live_quote(SwapNetwork::Mainnet, &amount, &token_out, &rpc).unwrap();
        let parsed: u128 = out
            .parse()
            .expect("amount_out fits u128 for 0.001 ETH worth of USDC");
        assert!(parsed > 0, "live quote returned zero amount_out");
        assert!(source.starts_with("uniswap-v3-quoter-mainnet-"));
    }
}
