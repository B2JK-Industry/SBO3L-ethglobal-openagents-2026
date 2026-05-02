//! SBO3L Phase 3.1 — on-chain audit-chain anchoring.
//!
//! Periodically publish a digest of the SBO3L audit chain to a
//! deployed `AnchorRegistry` contract on Ethereum mainnet (and
//! optionally Optimism / Base / Polygon via the same calldata).
//! The contract stores `(audit_root, chain_head_seq, ts)` triples;
//! a third-party auditor can later recompute the audit-root
//! locally and assert it matches the on-chain anchor at the time
//! the receipt was issued.
//!
//! # Why anchor
//!
//! SBO3L's audit chain is a hash-linked log signed by the daemon's
//! audit signer. A third-party auditor verifying a Passport
//! capsule needs **two** trust roots:
//!
//! 1. The audit signer's pubkey — published in ENS text records
//!    (T-3-1 / T-3-2) and re-derivable from the receipt.
//! 2. **Some external attestation that the audit chain hasn't been
//!    rewritten** between the receipt being issued and the auditor
//!    fetching it. The signer pubkey alone doesn't catch a daemon
//!    operator who re-signs a tampered chain — they hold the same
//!    key.
//!
//! On-chain anchoring fills slot 2: a periodic `writeAnchor` tx
//! commits `(audit_root, chain_head_seq, block.timestamp)` to a
//! public registry. Any chain rewrite after the anchor would
//! require either (a) a fresh anchor with the rewritten root
//! (publicly visible delta) or (b) reorganising the registry
//! contract's storage (impossible for an immutable mapping).
//!
//! # Wire shape
//!
//! ## `AuditAnchorEnvelope` — off-chain dry-run output
//!
//! ```json
//! {
//!   "schema": "sbo3l.audit_anchor_envelope.v1",
//!   "network": "mainnet",
//!   "registry_address": "0x...",
//!   "audit_root": "0x<32-byte-hex>",
//!   "chain_head_seq": 42,
//!   "chain_head_event_hash": "0x<32-byte-hex>",
//!   "computed_at": "2026-05-02T10:00:00Z",
//!   "writeAnchor_calldata_hex": "0x...",
//!   "broadcasted": false
//! }
//! ```
//!
//! ## `AnchorRegistry.writeAnchor(bytes32 auditRoot, uint64 chainHeadSeq)`
//!
//! Selector = first 4 bytes of
//! `keccak256("writeAnchor(bytes32,uint64)")` — pinned at
//! [`WRITE_ANCHOR_SELECTOR`] and re-derived in unit tests so a
//! drift can't slip through.
//!
//! The contract itself is a thin wrapper: a mapping from
//! `keccak256(audit_root || chain_head_seq)` → `block.timestamp`,
//! emitting an `AnchorWritten(audit_root, seq, ts)` event for
//! indexer consumption. Source ships in
//! `crates/sbo3l-anchor/contracts/AnchorRegistry.sol` as the
//! follow-up that handles the actual deployment.
//!
//! # Status
//!
//! This crate ships the off-chain digest builder + calldata
//! encoder + dry-run envelope. `--broadcast` is the natural
//! follow-up (mirrors the T-3-1 dry-run/broadcast pattern in
//! `sbo3l-cli/src/agent_broadcast.rs`).

pub mod digest;
pub mod registry;

pub use digest::{audit_root, AuditRootError};
pub use registry::{
    build_dry_run_envelope, write_anchor_calldata, AnchorRegistryError, AuditAnchorEnvelope,
    AuditAnchorNetwork, ANCHOR_REGISTRY_MAINNET, ANCHOR_REGISTRY_SEPOLIA,
    AUDIT_ANCHOR_ENVELOPE_SCHEMA, WRITE_ANCHOR_SELECTOR,
};
