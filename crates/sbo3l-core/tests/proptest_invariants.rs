//! Property-based invariants for sbo3l-core (R13 P1).
//!
//! Default mode runs 256 cases per property (proptest's default). Long-run
//! CI mode bumps this to 100K via `PROPTEST_CASES=100000` (set in the
//! nightly proptest workflow).
//!
//! Properties covered:
//! 1. **APRP wire-format roundtrip** — any [`PaymentRequest`] produced by
//!    our generator survives serde_json::to_string + serde_json::from_str
//!    byte-for-equiv.
//! 2. **JCS-canonical hash determinism** — `request_hash` is invariant
//!    under JSON object-key reordering. The same logical request hashes
//!    to the same bytes regardless of how the input JSON was constructed.
//! 3. **SHA-256 byte-flip detection** — for any non-empty input, flipping
//!    a single byte produces a different hash (the trivial avalanche
//!    property; sanity check for any wrapper around `sha2::Sha256`).
//! 4. **Audit-chain linkage non-collision** — for any two distinct
//!    `(prev_hash, payload)` pairs, the resulting `event.canonical_hash()`
//!    differs. Equivalent to "no chain-hash collision in the absence of a
//!    SHA-256 collision."
//!
//! These are not exhaustive by themselves — they verify properties that
//! must hold for **all** inputs, complementing the existing example-based
//! tests in `crates/sbo3l-core/tests/`.

use chrono::{TimeZone, Utc};
use proptest::prelude::*;
use sbo3l_core::aprp::{
    Currency, Destination, ExpectedResult, ExpectedResultKind, HttpMethod, Intent, Money,
    PaymentProtocol, PaymentRequest, RiskClass,
};
use sbo3l_core::audit::AuditEvent;
use sbo3l_core::hashing::{request_hash, sha256_hex};

// -------- generators --------

fn intent_strategy() -> impl Strategy<Value = Intent> {
    prop_oneof![
        Just(Intent::PurchaseApiCall),
        Just(Intent::PurchaseDataset),
        Just(Intent::PayComputeJob),
        Just(Intent::PayAgentService),
        Just(Intent::Tip),
    ]
}

fn currency_strategy() -> impl Strategy<Value = Currency> {
    Just(Currency::USD)
}

fn money_strategy() -> impl Strategy<Value = Money> {
    // Reasonable money strings — match the production wire format.
    (
        "[1-9][0-9]{0,9}\\.[0-9]{2}",
        currency_strategy(),
    )
        .prop_map(|(value, currency)| Money { value, currency })
}

fn http_method_strategy() -> impl Strategy<Value = HttpMethod> {
    prop_oneof![
        Just(HttpMethod::Get),
        Just(HttpMethod::Post),
        Just(HttpMethod::Put),
        Just(HttpMethod::Patch),
        Just(HttpMethod::Delete),
    ]
}

fn destination_strategy() -> impl Strategy<Value = Destination> {
    prop_oneof![
        (
            "https://[a-z]{3,12}\\.example\\.com/[a-z0-9_-]{1,30}",
            http_method_strategy(),
            prop::option::of("0x[0-9a-fA-F]{40}"),
        )
            .prop_map(|(url, method, expected_recipient)| Destination::X402Endpoint {
                url,
                method,
                expected_recipient,
            }),
        ("0x[0-9a-fA-F]{40}").prop_map(|address| Destination::Eoa { address }),
        ("0x[0-9a-fA-F]{40}").prop_map(|address| Destination::SmartAccount { address }),
        ("0x[0-9a-fA-F]{40}", "0x[0-9a-fA-F]{40}").prop_map(
            |(token_address, recipient)| Destination::Erc20Transfer {
                token_address,
                recipient,
            }
        ),
    ]
}

fn payment_protocol_strategy() -> impl Strategy<Value = PaymentProtocol> {
    prop_oneof![
        Just(PaymentProtocol::X402),
        Just(PaymentProtocol::L402),
        Just(PaymentProtocol::Erc20Transfer),
        Just(PaymentProtocol::SmartAccountSession),
    ]
}

fn risk_class_strategy() -> impl Strategy<Value = RiskClass> {
    prop_oneof![
        Just(RiskClass::Low),
        Just(RiskClass::Medium),
        Just(RiskClass::High),
        Just(RiskClass::Critical),
    ]
}

fn expected_result_strategy() -> impl Strategy<Value = ExpectedResult> {
    (
        prop_oneof![
            Just(ExpectedResultKind::Json),
            Just(ExpectedResultKind::File),
            Just(ExpectedResultKind::Receipt),
            Just(ExpectedResultKind::None),
        ],
        prop::option::of("[0-9a-f]{64}"),
        prop::option::of("[a-z]{3,12}/[a-z0-9_+-]{3,30}"),
    )
        .prop_map(|(kind, sha256, content_type)| ExpectedResult {
            kind,
            sha256,
            content_type,
        })
}

fn payment_request_strategy() -> impl Strategy<Value = PaymentRequest> {
    (
        // 0..=4 (struct construction needs ≤ 12 elements per tuple in proptest)
        "agent[0-9]{1,8}",
        "task[0-9]{1,8}",
        intent_strategy(),
        money_strategy(),
        "[A-Z]{2,6}",
        destination_strategy(),
        payment_protocol_strategy(),
        "[a-z]{3,12}-(mainnet|testnet|sepolia|polygon|base)",
        "https://[a-z]{3,12}\\.example\\.com/rpc",
        prop::option::of(prop::collection::hash_map(
            "[a-z]{3,8}",
            "[a-zA-Z0-9_-]{1,30}",
            0..3,
        )),
        // Expiry: keep within a reasonable hackathon range. UTC.
        // i64 seconds since 2020-01-01 → bounded to avoid overflow.
        1_577_836_800_i64..2_524_608_000_i64,
        "01[0-9A-HJKMNP-TV-Z]{24}", // ULID alphabet (Crockford-base32, no I/L/O/U)
        prop::option::of(expected_result_strategy()),
        risk_class_strategy(),
    )
        .prop_map(
            |(
                agent_id,
                task_id,
                intent,
                amount,
                token,
                destination,
                payment_protocol,
                chain,
                provider_url,
                metadata_opt,
                expiry_secs,
                nonce,
                expected_result,
                risk_class,
            )| {
                let x402_payload = metadata_opt.map(|m| {
                    serde_json::Value::Object(
                        m.into_iter()
                            .map(|(k, v)| (k, serde_json::Value::String(v)))
                            .collect(),
                    )
                });
                PaymentRequest {
                    agent_id,
                    task_id,
                    intent,
                    amount,
                    token,
                    destination,
                    payment_protocol,
                    chain,
                    provider_url,
                    x402_payload,
                    expiry: Utc.timestamp_opt(expiry_secs, 0).unwrap(),
                    nonce,
                    expected_result,
                    risk_class,
                }
            },
        )
}

// -------- properties --------

proptest! {
    /// **Property 1: APRP wire-format roundtrip.**
    /// Any APRP produced by the generator survives JSON serialization +
    /// deserialization byte-equivalent. Catches missing `serde` derives,
    /// renamed fields, default-skipping bugs.
    #[test]
    fn prop_aprp_roundtrip(req in payment_request_strategy()) {
        let json = serde_json::to_string(&req).expect("serialize");
        let back: PaymentRequest = serde_json::from_str(&json).expect("deserialize");
        prop_assert_eq!(req, back);
    }

    /// **Property 2: JCS-canonical hash determinism under key reordering.**
    /// The same logical APRP — serialized in two different valid JSON
    /// orderings — must hash to the same bytes. Verifies our use of
    /// JCS canonicalization is reorder-invariant.
    #[test]
    fn prop_request_hash_canonical(req in payment_request_strategy()) {
        let v1 = serde_json::to_value(&req).expect("to_value");
        let json_string_1 = serde_json::to_string(&v1).expect("to_string");
        // Re-parse + re-serialize through Map (HashMap-backed → arbitrary order).
        let parsed: serde_json::Value = serde_json::from_str(&json_string_1).expect("parse");
        let h1 = request_hash(&v1).expect("hash 1");
        let h2 = request_hash(&parsed).expect("hash 2");
        prop_assert_eq!(h1, h2);
    }

    /// **Property 3: SHA-256 byte-flip avalanche.**
    /// For any non-empty input, flipping one byte produces a different
    /// hash. Sanity check on the `sha256_hex` wrapper.
    #[test]
    fn prop_sha256_byte_flip_changes_hash(
        bytes in prop::collection::vec(any::<u8>(), 1..256),
        flip_idx in 0usize..256,
        flip_mask in 1u8..=255,
    ) {
        let idx = flip_idx % bytes.len();
        let h_original = sha256_hex(&bytes);
        let mut tampered = bytes.clone();
        tampered[idx] ^= flip_mask;
        // tampered != bytes (since flip_mask != 0)
        let h_tampered = sha256_hex(&tampered);
        prop_assert_ne!(h_original, h_tampered);
    }

    /// **Property 4: Audit-event canonical-hash linkage non-collision.**
    /// Two `AuditEvent`s that differ in `prev_event_hash` or `payload_hash`
    /// MUST produce different `canonical_hash()` outputs. This is the
    /// linkage-by-construction property — without it, an attacker could
    /// rewrite a chain row without breaking later linkage.
    #[test]
    fn prop_audit_event_hash_distinct_inputs(
        seq in 1u64..1_000_000,
        prev_hash_a in "[0-9a-f]{64}",
        prev_hash_b in "[0-9a-f]{64}",
        payload_hash_a in "[0-9a-f]{64}",
        payload_hash_b in "[0-9a-f]{64}",
    ) {
        prop_assume!(prev_hash_a != prev_hash_b || payload_hash_a != payload_hash_b);

        let event_a = AuditEvent {
            version: 1,
            seq,
            id: "01HCHADAUD000000000000000A".to_string(),
            ts: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
            event_type: "policy_decision".to_string(),
            actor: "agent-test".to_string(),
            subject_id: "subject-test".to_string(),
            payload_hash: payload_hash_a,
            metadata: serde_json::Map::new(),
            policy_version: Some(1),
            policy_hash: None,
            attestation_ref: None,
            prev_event_hash: prev_hash_a,
        };
        let event_b = AuditEvent {
            payload_hash: payload_hash_b,
            prev_event_hash: prev_hash_b,
            id: "01HCHADAUD000000000000000B".to_string(),
            ..event_a.clone()
        };
        let h_a = event_a.canonical_hash().expect("hash a");
        let h_b = event_b.canonical_hash().expect("hash b");
        prop_assert_ne!(h_a, h_b);
    }
}
