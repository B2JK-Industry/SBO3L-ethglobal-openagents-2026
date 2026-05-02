//! SBO3L browser playground bundle (R17 P1).
//!
//! Sister crate to `sbo3l-core`'s wasm verifier (#110). Where the
//! verifier is read-only ("does this capsule pass?"), the playground
//! is an *engine* — given an APRP and a policy, it returns a decision
//! and (optionally) builds a fully self-contained
//! `sbo3l.passport_capsule.v2` capsule whose 6-check strict verifier
//! passes with no auxiliary input.
//!
//! Bundle path on the marketing site:
//! `apps/marketing/public/wasm/sbo3l_playground_bg.wasm` plus
//! `sbo3l_playground.js`. The JS surface is exported via wasm-bindgen.
//!
//! # Why a separate crate
//!
//! `sbo3l-policy` already depends on `sbo3l-core`. A direct
//! `sbo3l-core → sbo3l-policy` dep edge would create a cycle even on
//! wasm32-only targets (cargo's resolver doesn't gate on
//! `cfg(target_arch)`). This crate sits below both: it depends on
//! both crates, calls `sbo3l_policy::decide`, and wraps the result in
//! a v2 capsule built via `passport_offline`. No native crate has
//! to depend on `sbo3l-playground`; only the wasm-pack pipeline does.
//!
//! # Public surface
//!
//! - [`passport_offline::build_capsule_v2_self_contained`] — pure
//!   Rust (no wasm-bindgen) builder. Native unit tests exercise the
//!   strict verifier against its output.
//! - [`wasm::decide_aprp_wasm`] (wasm32-only) — JS-callable real
//!   policy engine.
//! - [`wasm::build_capsule_wasm`] (wasm32-only) — JS-callable
//!   capsule builder using a caller-supplied Ed25519 seed.

pub mod passport_offline;

#[cfg(target_arch = "wasm32")]
pub mod wasm;
