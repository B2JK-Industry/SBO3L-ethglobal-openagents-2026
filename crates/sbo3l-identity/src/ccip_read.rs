//! CCIP-Read (ENSIP-10 / EIP-3668) client-side decoder + orchestrator.
//!
//! The gateway lives in `apps/ccip-gateway/` (TypeScript / Vercel
//! function); this module is the **Rust counterpart** for SBO3L's own
//! tooling — `sbo3l passport resolve <name>`, `sbo3l agent verify-ens`,
//! and friends — so we can resolve subnames behind the OffchainResolver
//! without trusting third-party clients.
//!
//! T-4-1 ships:
//!
//! * [`parse_offchain_lookup_revert`] — extract sender / urls /
//!   callData / callbackFunction / extraData from the standard
//!   `OffchainLookup(address,string[],bytes,bytes4,bytes)` revert
//!   payload that an OffchainResolver returns from `resolve()`.
//! * [`decode_gateway_response_body`] — parse the gateway's
//!   `{"data": "0x...", "ttl": N}` JSON.
//! * [`decode_gateway_data`] — split the gateway's `data` field into
//!   `(value, expires, signature)`.
//! * [`decode_string_result`] — pull the actual string out of
//!   `value`, which is the ABI-encoded `(string)` tuple that
//!   `abi.decode(..., (string))` would unwrap.
//!
//! Loop-7 follow-up (`sbo3l agent verify-ens` UAT): adds the missing
//! end-to-end follow:
//!
//! * [`substitute_gateway_url`] — substitute `{sender}` and `{data}`
//!   placeholders in the URL template per ENSIP-10.
//! * [`encode_callback_call`] — build the resolver's
//!   `callback(bytes response, bytes extraData)` calldata.
//! * [`follow_offchain_lookup`] — full follow: gateway GET → decode →
//!   callback `eth_call` → unwrap inner ABI tuple. Used by
//!   [`crate::ens_live::LiveEnsResolver::resolve_raw_text`] when the
//!   resolved name is behind an ENSIP-10 OffchainResolver.
//!
//! Signature verification (recover the gateway signer's address from
//! the signature, compare to the OffchainResolver's expected signer)
//! happens **on-chain** in `resolveCallback`. We don't re-verify in
//! Rust — the contract is the trust root. If the gateway returns a
//! tampered response, the on-chain callback reverts, our `eth_call`
//! surfaces that revert, and we bubble it up.

use serde::Deserialize;
use thiserror::Error;

use crate::ens_live::{JsonRpcTransport, RpcError};

/// Standard ENSIP-10 / EIP-3668 OffchainLookup error selector =
/// keccak256("OffchainLookup(address,string[],bytes,bytes4,bytes)")[..4].
pub const OFFCHAIN_LOOKUP_SELECTOR: [u8; 4] = [0x55, 0x6f, 0x18, 0x30];

#[derive(Debug, Error)]
pub enum CcipError {
    #[error("revert payload too short: {0} bytes")]
    RevertTooShort(usize),

    #[error("revert selector did not match OffchainLookup")]
    NotOffchainLookup,

    #[error("ABI decode error: {0}")]
    AbiDecode(String),

    #[error("gateway response JSON parse: {0}")]
    Json(#[from] serde_json::Error),

    #[error("hex decode: {0}")]
    Hex(#[from] hex::FromHexError),

    #[error("gateway data must be hex with `0x` prefix")]
    MissingHexPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffchainLookup {
    pub sender: [u8; 20],
    pub urls: Vec<String>,
    pub call_data: Vec<u8>,
    pub callback_selector: [u8; 4],
    pub extra_data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayResponse {
    pub value: Vec<u8>,
    pub expires: u64,
    pub signature: Vec<u8>,
}

/// Raw shape returned by the gateway. Distinct from
/// [`GatewayResponse`] which is the decoded inner ABI tuple.
#[derive(Debug, Deserialize)]
pub struct GatewayBody {
    pub data: String,
    #[allow(dead_code)]
    pub ttl: u64,
}

/// Parse the revert bytes of a `text(node, key)` or `addr(node)`
/// `eth_call` against an OffchainResolver. The first 4 bytes are the
/// OffchainLookup error selector; the remainder is ABI-encoded args.
pub fn parse_offchain_lookup_revert(revert: &[u8]) -> Result<OffchainLookup, CcipError> {
    if revert.len() < 4 {
        return Err(CcipError::RevertTooShort(revert.len()));
    }
    if revert[..4] != OFFCHAIN_LOOKUP_SELECTOR {
        return Err(CcipError::NotOffchainLookup);
    }
    decode_lookup_args(&revert[4..])
}

fn decode_lookup_args(b: &[u8]) -> Result<OffchainLookup, CcipError> {
    // ABI layout (head section, 5 words = 5 * 32 bytes):
    //   word 0: address sender   (20 bytes left-padded)
    //   word 1: offset to urls   (string[])
    //   word 2: offset to callData (bytes)
    //   word 3: bytes4 callbackFunction (left-aligned in 32-byte slot)
    //   word 4: offset to extraData (bytes)
    if b.len() < 5 * 32 {
        return Err(CcipError::AbiDecode(
            "lookup args head too short".to_string(),
        ));
    }
    let mut sender = [0u8; 20];
    sender.copy_from_slice(&b[12..32]);

    let urls_offset = read_u64_word(b, 32)?;
    let calldata_offset = read_u64_word(b, 64)?;
    let mut callback_selector = [0u8; 4];
    callback_selector.copy_from_slice(&b[96..100]);
    let extra_data_offset = read_u64_word(b, 128)?;

    let urls = decode_string_array(b, urls_offset as usize)?;
    let call_data = decode_bytes(b, calldata_offset as usize)?;
    let extra_data = decode_bytes(b, extra_data_offset as usize)?;

    Ok(OffchainLookup {
        sender,
        urls,
        call_data,
        callback_selector,
        extra_data,
    })
}

fn read_u64_word(b: &[u8], off: usize) -> Result<u64, CcipError> {
    if b.len() < off + 32 {
        return Err(CcipError::AbiDecode(format!("word at offset {off} OOB")));
    }
    // Last 8 bytes of the 32-byte big-endian word.
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&b[off + 24..off + 32]);
    Ok(u64::from_be_bytes(buf))
}

fn decode_bytes(b: &[u8], offset: usize) -> Result<Vec<u8>, CcipError> {
    if b.len() < offset + 32 {
        return Err(CcipError::AbiDecode(format!(
            "bytes len-word at {offset} OOB"
        )));
    }
    let len = read_u64_word(b, offset)? as usize;
    let start = offset + 32;
    let end = start + len;
    if b.len() < end {
        return Err(CcipError::AbiDecode(format!(
            "bytes content at {start}..{end} OOB"
        )));
    }
    Ok(b[start..end].to_vec())
}

fn decode_string_array(b: &[u8], offset: usize) -> Result<Vec<String>, CcipError> {
    if b.len() < offset + 32 {
        return Err(CcipError::AbiDecode("string[] len-word OOB".to_string()));
    }
    let len = read_u64_word(b, offset)? as usize;
    let head_start = offset + 32;
    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let head_off = head_start + i * 32;
        let elem_off = read_u64_word(b, head_off)? as usize + head_start;
        let bytes = decode_bytes(b, elem_off)?;
        out.push(
            String::from_utf8(bytes)
                .map_err(|e| CcipError::AbiDecode(format!("non-utf8 url: {e}")))?,
        );
    }
    Ok(out)
}

/// Parse the gateway's JSON body to a [`GatewayBody`].
pub fn decode_gateway_response_body(body: &[u8]) -> Result<GatewayBody, CcipError> {
    Ok(serde_json::from_slice(body)?)
}

/// Decode the gateway's `data` field into the
/// `(bytes value, uint64 expires, bytes signature)` triple.
pub fn decode_gateway_data(data_hex: &str) -> Result<GatewayResponse, CcipError> {
    let stripped = data_hex
        .strip_prefix("0x")
        .or_else(|| data_hex.strip_prefix("0X"))
        .ok_or(CcipError::MissingHexPrefix)?;
    let raw = hex::decode(stripped)?;

    // ABI head: 3 words = 96 bytes.
    //   word 0: offset to value
    //   word 1: uint64 expires (right-padded in 32-byte slot)
    //   word 2: offset to signature
    if raw.len() < 96 {
        return Err(CcipError::AbiDecode(
            "gateway data head too short".to_string(),
        ));
    }
    let value_offset = read_u64_word(&raw, 0)? as usize;
    let expires = read_u64_word(&raw, 32)?;
    let signature_offset = read_u64_word(&raw, 64)? as usize;

    let value = decode_bytes(&raw, value_offset)?;
    let signature = decode_bytes(&raw, signature_offset)?;

    Ok(GatewayResponse {
        value,
        expires,
        signature,
    })
}

/// Substitute `{sender}` and `{data}` placeholders in a CCIP-Read
/// gateway URL template per EIP-3668 / ENSIP-10. Both substitutions
/// are 0x-prefixed lowercase hex of the corresponding bytes.
///
/// `sender` is the OffchainResolver's address (20 bytes).
/// `call_data` is the original inner calldata (the `text(node, key)`
/// or `addr(node)` selector + ABI args that the resolver was asked
/// to handle).
pub fn substitute_gateway_url(template: &str, sender: &[u8; 20], call_data: &[u8]) -> String {
    let sender_hex = format!("0x{}", hex::encode(sender));
    let data_hex = format!("0x{}", hex::encode(call_data));
    template
        .replace("{sender}", &sender_hex)
        .replace("{data}", &data_hex)
}

/// Encode the resolver-side callback calldata: `selector ||
/// abi.encode(bytes response, bytes extraData)`.
///
/// `response` is the gateway's signed `(value, expires, signature)`
/// triple (raw bytes — already ABI-encoded by the gateway). The
/// resolver re-decodes it inside `resolveCallback` and verifies the
/// signature on-chain.
pub fn encode_callback_call(
    callback_selector: &[u8; 4],
    response: &[u8],
    extra_data: &[u8],
) -> Vec<u8> {
    // Two dynamic args (`bytes response, bytes extraData`):
    //   head[0] = offset to response (= 0x40, after the two heads)
    //   head[1] = offset to extraData (= 0x40 + 32 + padded(response))
    //   tail[0] = u256 length || padded response bytes
    //   tail[1] = u256 length || padded extra_data bytes
    let response_padded = response.len().div_ceil(32) * 32;
    let extra_data_padded = extra_data.len().div_ceil(32) * 32;

    let head_offset_response: u64 = 0x40;
    let head_offset_extra: u64 = head_offset_response + 32 + response_padded as u64;

    let mut out = Vec::with_capacity(4 + 64 + 32 + response_padded + 32 + extra_data_padded);
    out.extend_from_slice(callback_selector);

    // Heads.
    out.extend_from_slice(&u256_be(head_offset_response));
    out.extend_from_slice(&u256_be(head_offset_extra));

    // response tail.
    out.extend_from_slice(&u256_be(response.len() as u64));
    out.extend_from_slice(response);
    out.extend(std::iter::repeat_n(0u8, response_padded - response.len()));

    // extra_data tail.
    out.extend_from_slice(&u256_be(extra_data.len() as u64));
    out.extend_from_slice(extra_data);
    out.extend(std::iter::repeat_n(
        0u8,
        extra_data_padded - extra_data.len(),
    ));

    out
}

fn u256_be(n: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&n.to_be_bytes());
    out
}

/// CCIP-Read end-to-end follow: gateway request → decode response →
/// callback `eth_call` on the OffchainResolver → unwrap the inner
/// `bytes` return, which is the ABI-encoded payload the original
/// `text(node, key)` / `addr(node)` would have returned (i.e. an
/// ABI-encoded `(string)` for `text`, or a 32-byte address word for
/// `addr`).
///
/// Per EIP-3668 §"Client Lookup Protocol":
///
/// * **HTTP method.** If the URL template contains the `{data}`
///   placeholder, use GET (params in URL). Otherwise POST a JSON
///   body `{"sender":"0x..","data":"0x.."}`. Both forms are
///   compliant; the resolver picks per-URL.
/// * **Retry classification.** 4xx responses are *terminal* for that
///   lookup — the gateway has authoritative data and is rejecting,
///   so trying another gateway won't help. 5xx and transport errors
///   trigger trying the next URL in the list.
///
/// Returns the inner-result bytes. Use [`decode_string_result`] to
/// pull a string out of a `text(node, key)` follow.
pub fn follow_offchain_lookup<T: JsonRpcTransport>(
    transport: &T,
    lookup: &OffchainLookup,
) -> Result<Vec<u8>, RpcError> {
    if lookup.urls.is_empty() {
        return Err(RpcError::Decode(
            "OffchainLookup carried no gateway URLs".into(),
        ));
    }

    let mut last_error: Option<RpcError> = None;
    for template in &lookup.urls {
        let has_data_placeholder = template.contains("{data}");
        let url = substitute_gateway_url(template, &lookup.sender, &lookup.call_data);

        let body_result = if has_data_placeholder {
            transport.http_get(&url)
        } else {
            let body = serde_json::json!({
                "sender": format!("0x{}", hex::encode(lookup.sender)),
                "data": format!("0x{}", hex::encode(&lookup.call_data)),
            })
            .to_string()
            .into_bytes();
            transport.http_post_json(&url, &body)
        };

        match body_result {
            Ok(body) => {
                let parsed = decode_gateway_response_body(&body)
                    .map_err(|e| RpcError::Decode(format!("gateway body {url}: {e}")))?;
                let response_hex = parsed.data;
                let stripped = response_hex
                    .strip_prefix("0x")
                    .or_else(|| response_hex.strip_prefix("0X"))
                    .ok_or_else(|| {
                        RpcError::Decode(format!("gateway {url}: response.data missing 0x prefix"))
                    })?;
                let response_bytes = hex::decode(stripped).map_err(|e| {
                    RpcError::Decode(format!("gateway {url}: response.data hex decode: {e}"))
                })?;

                // Build the callback eth_call.
                let callback_data = encode_callback_call(
                    &lookup.callback_selector,
                    &response_bytes,
                    &lookup.extra_data,
                );
                let sender_hex = format!("0x{}", hex::encode(lookup.sender));
                let calldata_hex = format!("0x{}", hex::encode(&callback_data));
                let raw = transport.eth_call(&sender_hex, &calldata_hex)?;

                // The callback returns `bytes` — outer `eth_call`
                // result is the ABI-encoded single-bytes tuple.
                let outer = raw
                    .strip_prefix("0x")
                    .or_else(|| raw.strip_prefix("0X"))
                    .ok_or_else(|| {
                        RpcError::Decode(format!(
                            "callback {sender_hex}: response missing 0x prefix"
                        ))
                    })?;
                let outer_bytes = hex::decode(outer).map_err(|e| {
                    RpcError::Decode(format!("callback {sender_hex}: hex decode: {e}"))
                })?;
                return decode_single_bytes_tuple(&outer_bytes)
                    .map_err(|e| RpcError::Decode(format!("callback {sender_hex}: {e}")));
            }
            Err(e) => {
                // EIP-3668 §"Client Lookup Protocol": 4xx is
                // authoritative-rejection from the gateway and
                // terminates this lookup; 5xx (and transport
                // failures) trigger trying the next URL.
                let is_4xx = is_http_4xx(&e);
                last_error = Some(e);
                if is_4xx {
                    break;
                }
                // Else fall through and try the next URL in the list.
            }
        }
    }
    Err(last_error
        .unwrap_or_else(|| RpcError::Http("CCIP-Read: all gateway URLs exhausted".into())))
}

/// Detect a 4xx HTTP status from the error string the live transport
/// produces (`"status 4xx"`). The trait-level error is opaque, so we
/// match on the standardised substring rather than parameterising
/// every transport with a status code accessor.
fn is_http_4xx(e: &RpcError) -> bool {
    if let RpcError::Http(msg) = e {
        // Live transport formats: "...: status 404 Not Found".
        // Match digit immediately after "status ".
        if let Some(idx) = msg.find("status ") {
            let tail = &msg[idx + "status ".len()..];
            let mut iter = tail.chars();
            return matches!(iter.next(), Some('4'))
                && iter.next().is_some_and(|c| c.is_ascii_digit())
                && iter.next().is_some_and(|c| c.is_ascii_digit());
        }
    }
    false
}

/// Decode an ABI-encoded `(bytes)` tuple — the outer shape returned
/// by `resolveCallback(response, extraData) returns (bytes)`.
fn decode_single_bytes_tuple(b: &[u8]) -> Result<Vec<u8>, CcipError> {
    if b.len() < 64 {
        return Err(CcipError::AbiDecode(format!(
            "(bytes) tuple too short: {} bytes",
            b.len()
        )));
    }
    let offset = read_u64_word(b, 0)? as usize;
    decode_bytes(b, offset)
}

/// Decode an ABI-encoded `(string)` tuple as a UTF-8 [`String`]. The
/// gateway returns text records this way for `text(node, key)`
/// queries.
pub fn decode_string_result(result_bytes: &[u8]) -> Result<String, CcipError> {
    // ABI: head word = offset (always 0x20 for single-string),
    // followed by length + padded bytes.
    if result_bytes.len() < 64 {
        return Err(CcipError::AbiDecode("string result too short".to_string()));
    }
    let len = read_u64_word(result_bytes, 32)? as usize;
    let start = 64;
    let end = start + len;
    if result_bytes.len() < end {
        return Err(CcipError::AbiDecode("string content OOB".to_string()));
    }
    String::from_utf8(result_bytes[start..end].to_vec())
        .map_err(|e| CcipError::AbiDecode(format!("non-utf8: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Local minimal fake — covers just enough of `JsonRpcTransport`
    /// to drive `follow_offchain_lookup` end-to-end. The richer
    /// fixture in `ens_live::tests` exercises the resolver-side
    /// dispatch; this one focuses on the EIP-3668 client semantics
    /// (GET vs POST, 4xx vs 5xx).
    struct FollowFake {
        get_responses: RefCell<Vec<Result<Vec<u8>, RpcError>>>,
        post_responses: RefCell<Vec<Result<Vec<u8>, RpcError>>>,
        eth_call_responses: RefCell<Vec<Result<String, RpcError>>>,
        http_log: RefCell<Vec<String>>,
    }

    impl FollowFake {
        fn new() -> Self {
            Self {
                get_responses: RefCell::new(Vec::new()),
                post_responses: RefCell::new(Vec::new()),
                eth_call_responses: RefCell::new(Vec::new()),
                http_log: RefCell::new(Vec::new()),
            }
        }
    }

    impl JsonRpcTransport for FollowFake {
        fn eth_call(&self, _to: &str, _data: &str) -> Result<String, RpcError> {
            self.eth_call_responses
                .borrow_mut()
                .pop()
                .unwrap_or_else(|| Err(RpcError::Decode("fake: no eth_call scripted".into())))
        }
        fn http_get(&self, url: &str) -> Result<Vec<u8>, RpcError> {
            self.http_log.borrow_mut().push(format!("GET {url}"));
            self.get_responses
                .borrow_mut()
                .pop()
                .unwrap_or_else(|| Err(RpcError::Http(format!("fake: no GET scripted for {url}"))))
        }
        fn http_post_json(&self, url: &str, body: &[u8]) -> Result<Vec<u8>, RpcError> {
            self.http_log.borrow_mut().push(format!(
                "POST {url} body={}",
                std::str::from_utf8(body).unwrap_or("<non-utf8>")
            ));
            self.post_responses
                .borrow_mut()
                .pop()
                .unwrap_or_else(|| Err(RpcError::Http(format!("fake: no POST scripted for {url}"))))
        }
    }

    fn build_lookup(urls: &[&str]) -> OffchainLookup {
        OffchainLookup {
            sender: [0x11u8; 20],
            urls: urls.iter().map(|s| s.to_string()).collect(),
            call_data: vec![0xaa, 0xbb],
            callback_selector: [0xb4, 0xa8, 0x5b, 0x71],
            extra_data: vec![0xcc],
        }
    }

    /// EIP-3668 §"Client Lookup Protocol" (Codex P1 review on PR
    /// #446): when the URL template contains `{data}`, use HTTP GET.
    /// `follow_offchain_lookup` should *only* hit `http_get` on this
    /// path, never `http_post_json`.
    #[test]
    fn follow_uses_get_when_url_has_data_placeholder() {
        let fake = FollowFake::new();
        // Push a 4xx so the follow terminates without invoking the
        // callback eth_call — keeps the test focused on dispatch.
        fake.get_responses
            .borrow_mut()
            .push(Err(RpcError::Http("GET ...: status 404 Not Found".into())));
        let lookup = build_lookup(&["https://gw.test/api/{sender}/{data}.json"]);
        let _ = follow_offchain_lookup(&fake, &lookup);
        let log = fake.http_log.borrow();
        assert_eq!(log.len(), 1, "exactly one HTTP attempt");
        assert!(log[0].starts_with("GET "), "got: {log:?}");
    }

    /// EIP-3668: when the URL template lacks `{data}`, the client
    /// MUST POST `{"sender":"0x..","data":"0x.."}`. (Codex P1 review
    /// on PR #446 — previously we always GET-ed.)
    #[test]
    fn follow_uses_post_when_url_lacks_data_placeholder() {
        let fake = FollowFake::new();
        fake.post_responses
            .borrow_mut()
            .push(Err(RpcError::Http("POST ...: status 404 Not Found".into())));
        // Template has only {sender} → POST.
        let lookup = build_lookup(&["https://gw.test/api/{sender}/lookup"]);
        let _ = follow_offchain_lookup(&fake, &lookup);
        let log = fake.http_log.borrow();
        assert_eq!(log.len(), 1, "exactly one HTTP attempt");
        assert!(log[0].starts_with("POST "), "got: {log:?}");
        assert!(
            log[0].contains(r#""sender":"0x"#),
            "missing sender: {log:?}"
        );
        assert!(
            log[0].contains(r#""data":"0xaabb""#),
            "missing data: {log:?}"
        );
    }

    /// EIP-3668: 4xx responses are *terminal* — the gateway has
    /// authoritative data and is rejecting. Don't try the next URL.
    /// (Codex P2 review on PR #446 flagged the inverted policy.)
    #[test]
    fn follow_does_not_retry_after_4xx() {
        let fake = FollowFake::new();
        // Push *two* 404s, but we expect only the first to be
        // consumed (4xx terminates).
        fake.get_responses.borrow_mut().push(Err(RpcError::Http(
            "GET https://gw2.test/...: status 404 Not Found".into(),
        )));
        fake.get_responses.borrow_mut().push(Err(RpcError::Http(
            "GET https://gw1.test/...: status 404 Not Found".into(),
        )));
        let lookup = build_lookup(&[
            "https://gw1.test/api/{sender}/{data}.json",
            "https://gw2.test/api/{sender}/{data}.json",
        ]);
        let err = follow_offchain_lookup(&fake, &lookup).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("404"), "expected 404 in surfaced error: {msg}");
        assert_eq!(
            fake.http_log.borrow().len(),
            1,
            "must NOT try next URL after a 4xx"
        );
    }

    /// EIP-3668: 5xx responses + transport errors trigger the next
    /// URL. (Codex P2 review on PR #446 flagged the inverted policy.)
    #[test]
    fn follow_retries_next_url_after_5xx() {
        let fake = FollowFake::new();
        // Pre-populate post-stack-pop order: first GET (gw1) =
        // 502, second GET (gw2) = success.
        let success_body = serde_json::json!({
            "data": "0x",
            "ttl": 60,
        })
        .to_string()
        .into_bytes();
        fake.get_responses.borrow_mut().push(Ok(success_body));
        fake.get_responses.borrow_mut().push(Err(RpcError::Http(
            "GET https://gw1.test/...: status 502 Bad Gateway".into(),
        )));
        // The success path then tries to decode a callback eth_call.
        // We don't care about the callback in this test (it'll fail
        // on empty `0x`); we just want to confirm we tried both
        // URLs.
        fake.eth_call_responses
            .borrow_mut()
            .push(Err(RpcError::Decode("test stub".into())));

        let lookup = build_lookup(&[
            "https://gw1.test/api/{sender}/{data}.json",
            "https://gw2.test/api/{sender}/{data}.json",
        ]);
        let _ = follow_offchain_lookup(&fake, &lookup);
        let log = fake.http_log.borrow();
        assert_eq!(log.len(), 2, "should try both URLs after 5xx");
        assert!(
            log[0].contains("gw1.test"),
            "first attempt should be gw1: {log:?}"
        );
        assert!(
            log[1].contains("gw2.test"),
            "second attempt should be gw2 after 5xx: {log:?}"
        );
    }

    #[test]
    fn offchain_lookup_selector_matches_keccak() {
        // Recompute and assert vs the const so we never silently drift.
        use tiny_keccak::{Hasher, Keccak};
        let mut h = Keccak::v256();
        h.update(b"OffchainLookup(address,string[],bytes,bytes4,bytes)");
        let mut out = [0u8; 32];
        h.finalize(&mut out);
        assert_eq!(&out[..4], &OFFCHAIN_LOOKUP_SELECTOR);
    }

    #[test]
    fn parse_short_revert_returns_too_short() {
        let err = parse_offchain_lookup_revert(&[0x55, 0x6f]).unwrap_err();
        assert!(matches!(err, CcipError::RevertTooShort(2)));
    }

    #[test]
    fn parse_wrong_selector_returns_not_offchain_lookup() {
        let bytes = vec![0xde, 0xad, 0xbe, 0xef, 0u8, 0u8];
        let err = parse_offchain_lookup_revert(&bytes).unwrap_err();
        assert!(matches!(err, CcipError::NotOffchainLookup));
    }

    #[test]
    fn decode_gateway_response_body_happy() {
        let body = br#"{"data":"0x1234","ttl":60}"#;
        let parsed = decode_gateway_response_body(body).unwrap();
        assert_eq!(parsed.data, "0x1234");
        assert_eq!(parsed.ttl, 60);
    }

    #[test]
    fn decode_gateway_response_body_rejects_garbage() {
        let body = br#"not json"#;
        let err = decode_gateway_response_body(body).unwrap_err();
        assert!(matches!(err, CcipError::Json(_)));
    }

    #[test]
    fn decode_gateway_data_rejects_no_prefix() {
        let err = decode_gateway_data("1234").unwrap_err();
        assert!(matches!(err, CcipError::MissingHexPrefix));
    }

    #[test]
    fn decode_gateway_data_rejects_short() {
        let err = decode_gateway_data("0x00").unwrap_err();
        assert!(matches!(err, CcipError::Hex(_) | CcipError::AbiDecode(_)));
    }

    #[test]
    fn substitute_gateway_url_replaces_both_placeholders() {
        let template = "https://example.test/api/{sender}/{data}.json";
        let sender = [0x11u8; 20];
        let call_data = vec![0xaa, 0xbb, 0xcc];
        let url = substitute_gateway_url(template, &sender, &call_data);
        assert_eq!(
            url,
            "https://example.test/api/0x1111111111111111111111111111111111111111/0xaabbcc.json"
        );
    }

    #[test]
    fn substitute_gateway_url_idempotent_when_placeholders_missing() {
        let template = "https://no-placeholders.test/static.json";
        let sender = [0u8; 20];
        let url = substitute_gateway_url(template, &sender, &[]);
        assert_eq!(url, template);
    }

    #[test]
    fn encode_callback_call_starts_with_selector() {
        let selector = [0xde, 0xad, 0xbe, 0xef];
        let response = vec![0x01, 0x02, 0x03];
        let extra = vec![0xff];
        let bytes = encode_callback_call(&selector, &response, &extra);
        assert_eq!(&bytes[..4], &selector);
        // Two head words = 64 bytes after selector.
        assert_eq!(bytes.len() - 4, 64 + 32 + 32 + 32 + 32);
    }

    #[test]
    fn encode_callback_call_offsets_resolve_to_payloads() {
        let selector = [0u8; 4];
        let response = b"response-bytes-payload".to_vec();
        let extra = b"extra".to_vec();
        let bytes = encode_callback_call(&selector, &response, &extra);
        // Skip selector. Head[0] = offset to response = 0x40.
        let body = &bytes[4..];
        let head0 = read_u64_word(body, 0).unwrap() as usize;
        let head1 = read_u64_word(body, 32).unwrap() as usize;
        assert_eq!(head0, 0x40);
        // Response length word at offset 0x40.
        let resp_len = read_u64_word(body, head0).unwrap() as usize;
        assert_eq!(resp_len, response.len());
        // Response bytes at offset 0x40 + 32.
        assert_eq!(&body[head0 + 32..head0 + 32 + resp_len], &response[..]);
        // Extra length word at head1.
        let extra_len = read_u64_word(body, head1).unwrap() as usize;
        assert_eq!(extra_len, extra.len());
        assert_eq!(&body[head1 + 32..head1 + 32 + extra_len], &extra[..]);
    }

    #[test]
    fn decode_single_bytes_tuple_round_trip() {
        // (bytes) = head(0x20) || length || padded body.
        let payload = b"hello-callback-result".to_vec();
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0u8; 31]);
        buf.push(0x20);
        let mut len_word = [0u8; 32];
        len_word[24..].copy_from_slice(&(payload.len() as u64).to_be_bytes());
        buf.extend_from_slice(&len_word);
        let pad = payload.len().div_ceil(32) * 32 - payload.len();
        buf.extend_from_slice(&payload);
        buf.extend(std::iter::repeat_n(0u8, pad));
        let decoded = decode_single_bytes_tuple(&buf).unwrap();
        assert_eq!(decoded, payload);
    }

    #[test]
    fn is_http_4xx_recognises_404_but_not_5xx_or_transport() {
        assert!(is_http_4xx(&RpcError::Http(
            "GET https://x: status 404 Not Found".into()
        )));
        assert!(is_http_4xx(&RpcError::Http(
            "POST https://x: status 410 Gone".into()
        )));
        assert!(is_http_4xx(&RpcError::Http(
            "GET https://x: status 451 Unavailable For Legal Reasons".into()
        )));
        // 5xx must not match (gateway-outage retry path).
        assert!(!is_http_4xx(&RpcError::Http(
            "GET https://x: status 500 Internal Server Error".into()
        )));
        assert!(!is_http_4xx(&RpcError::Http(
            "GET https://x: status 503 Service Unavailable".into()
        )));
        // Plain transport failure (no "status N" substring) must
        // NOT be classified as 4xx — those are retry-next-URL.
        assert!(!is_http_4xx(&RpcError::Http("connection refused".into())));
        // Other RpcError variants don't classify either.
        assert!(!is_http_4xx(&RpcError::Decode("garbage".into())));
    }

    #[test]
    fn decode_string_result_round_trip() {
        // Hand-built ABI-encoded tuple: (string).
        // head: 0x20 (offset)
        // tail: length=5, "hello", padded to 32
        let mut buf = vec![0u8; 32];
        buf[31] = 0x20; // offset = 32
                        // length = 5
        let mut len_word = [0u8; 32];
        len_word[31] = 5;
        buf.extend_from_slice(&len_word);
        // "hello" + 27 bytes padding
        buf.extend_from_slice(b"hello");
        buf.extend_from_slice(&[0u8; 27]);
        let s = decode_string_result(&buf).unwrap();
        assert_eq!(s, "hello");
    }
}
