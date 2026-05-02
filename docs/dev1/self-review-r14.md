# Dev 1 — R14 self-review: bugs found in my own PRs

**Authored:** 2026-05-02T17:10Z
**Method:** pulled the unified diff of every R14 PR I authored (#322 #323 #324 #327 #329 #330), grep-scanned for known anti-patterns (Crockford violations, production `unwrap()`, format-string SQL, hardcoded secrets, `unsafe` blocks, missing low-S normalisation, unbounded buffers), then read each match in context.
**Verdict:** **2 real bugs, 1 architectural concern, 0 security issues.** Each is named with severity, the fix, and where the fix should land.

---

## Bug 1 — `sbo3l admin verify` is incomplete tamper-evidence (PR #327)

**Severity:** medium. The PR description and module docs claim **"tamper-evidence"** for the verify path, but the implementation only checks `prev_event_hash` linkage. A sophisticated attacker who rewrites BOTH `prev_event_hash` AND the event payload + recomputed `event_hash` consistently would pass the current check.

**What the code does (`crates/sbo3l-cli/src/admin_backup.rs::cmd_verify`):**

```rust
const GENESIS_PREV: &str = "0000…0000";
let mut prev_hash = String::new();
for (i, evt) in events.iter().enumerate() {
    let want_prev = if i == 0 { GENESIS_PREV } else { prev_hash.as_str() };
    if evt.event.prev_event_hash != want_prev { return ExitCode::from(1); }
    prev_hash = evt.event_hash.clone();
}
```

**What it SHOULD do:** call `sbo3l_core::audit::verify_chain(&events, true, signer_pubkey)` which already exists and additionally:
- Re-canonicalises every event's payload, recomputes `event_hash`, asserts match (catches event-payload tampering even when `prev_event_hash` is rewritten consistently).
- If `signer_pubkey` is provided, verifies the Ed25519 signature on every event.

**Fix (one CLI flag + one function call):**

```rust
// In AdminCmd::Verify:
//   #[arg(long)]
//   audit_signer_pubkey: Option<String>,    // 64-char hex Ed25519 pubkey

// In cmd_verify, replace the manual loop with:
match sbo3l_core::audit::verify_chain(&events, /*verify_hashes=*/true, args.audit_signer_pubkey.as_deref()) {
    Ok(()) => {
        let suffix = if args.audit_signer_pubkey.is_some() { "incl. signatures" } else { "structural + hash, no signature pubkey supplied" };
        println!("✓ verified {count} audit events ({suffix})");
        ExitCode::SUCCESS
    }
    Err(e) => {
        eprintln!("sbo3l admin verify: chain verification failed: {e}");
        ExitCode::from(1)
    }
}
```

**Where to land:** the fix has to ride on top of PR #327 since the verify code only exists on that branch. Whoever rebases #327 should apply this patch. **A test must accompany:** mutate one byte of an event's payload in the seeded DB, re-run verify, assert non-zero exit + the chain error message — that's the regression test the current code would silently pass.

---

## Bug 2 — gRPC `AuditChainStream` loads the entire chain into memory (PR #322)

**Severity:** medium. DoS surface. `crates/sbo3l-server/src/grpc.rs::audit_chain_stream` does:

```rust
let events: Vec<SignedAuditEvent> = {
    let storage = self.inner.storage.lock()...;
    storage.audit_list()?    // FULL TABLE
};
let stream = async_stream::try_stream! {
    for ev in events.into_iter() {
        if ev.event.seq <= since_seq { continue; }    // skip post-load
        if emitted >= limit { break; }
        ...
    }
};
```

**Problem:** memory usage scales with TOTAL chain size, not the page size. A request with `since_seq=99000, limit=10` against a 100K-event chain still allocates ~100MB. A bad actor making concurrent paginated requests can exhaust server memory.

**Fix:** push `since_seq + limit` down into storage. `crates/sbo3l-storage/src/audit_store.rs` does NOT currently expose `audit_list_paginated(since_seq, limit)`; the closest primitives are `audit_list()` (everything) and `audit_chain_prefix_through(event_id)` (genesis-to-id). Adding `audit_list_paginated` is a small, focused addition:

```rust
pub fn audit_list_paginated(&self, since_seq: u64, limit: u64)
    -> StorageResult<Vec<SignedAuditEvent>>
{
    let mut stmt = self.conn.prepare(
        "SELECT … FROM audit_events WHERE seq > ?1 ORDER BY seq ASC LIMIT ?2"
    )?;
    stmt.query_map(params![since_seq as i64, limit as i64], row_to_signed_audit_event)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}
```

Then `audit_chain_stream` calls `audit_list_paginated(since_seq, limit.min(MAX_PAGE_SIZE))` instead of `audit_list()`.

**Where to land:** PR #322 is conflicted; the fix has to ride on the rebase. Storage primitive addition is a small main-line PR that can land independently.

---

## Architectural concern 1 — `VACUUM INTO` SQL string formatting (PR #327)

**Severity:** **NOT a bug** — flagging because the surface looks like SQL injection but isn't.

```rust
let dst_str = dst.to_string_lossy().replace('\'', "''");
conn.execute_batch(&format!("VACUUM INTO '{dst_str}';"))
```

`VACUUM INTO` cannot use bound parameters per SQLite parser semantics — the destination must be a string literal. The `replace('\'', "''")` doubles single quotes (the standard SQL string-literal escape), so a path like `O'Brien.db` becomes `O''Brien.db` correctly. Verified by inspection that:
- `to_string_lossy()` substitutes invalid UTF-8 with `U+FFFD` so no null-byte injection.
- Single quotes are the only special character inside a SQLite string literal that needs escaping.
- Embedded newlines / control chars are valid literal data inside `'…'`.

**Defence-in-depth nice-to-have (NOT a bug fix, just hygiene):** reject paths containing `\0`, control characters, or absurd lengths before formatting. Marginal value; the user controls the path through their CLI flag. Logging this here so it doesn't get re-flagged in a future audit.

---

## Items checked and clean

| Surface | Concern checked | Result |
|---|---|---|
| #324 KMS recovery byte / low-S | secp256k1 `normalize_s()` applied; recovery-id derived by trying `v=0,1` and matching expected pubkey | clean |
| #324 KMS env-var test access | uses `env_lock()` + `unsafe { std::env::set_var/remove_var }` (Rust 2024 edition requires `unsafe`) with explicit lock guard | clean |
| #330 OTEL `unsafe { libc::kill(pid, SIGTERM) }` | inside `#[cfg(unix)]`, FFI invariants documented in `// SAFETY:` comment, valid pid + valid signal | clean |
| #330 OTEL graceful shutdown | both `tokio::signal::ctrl_c()` and `SIGTERM` handled via `tokio::select!` | clean |
| #329 Helm pod security context | `runAsNonRoot: true`, `runAsUser: 65532`, `allowPrivilegeEscalation: false`, all Linux capabilities dropped | clean |
| #329 Helm image tag | `tag: "1.2.0"` pinned, not `latest` | clean |
| All 6 PRs — Crockford nonce rule (no I/L/O/U) | grep across all diffs found zero violations | clean |
| All 6 PRs — production `unwrap()` / `expect()` outside tests | grep with test-path filter found zero unwrap on hot paths | clean |
| All 6 PRs — `TODO`/`FIXME` in shipped code | only #323 (Raft, marked EXPERIMENTAL); pre-flagged in module docs | clean |

---

## Summary

| Finding | PR | Severity | Where to fix |
|---|---|---|---|
| `verify` only checks chain links, not payload-hash or signatures | #327 | medium | rebase + apply patch above |
| `audit_chain_stream` loads full chain before paging | #322 | medium | rebase + add `audit_list_paginated` to storage |
| `VACUUM INTO` format-with-escape (cosmetic) | #327 | none | optional defence-in-depth |

**Both real findings ride on PRs that are currently CONFLICTING vs main and need rebase.** The rebase is queued for the next session; this self-review captures the patches so the rebaser doesn't have to re-derive them. PR comments updated on #322 and #327 with pointers here.

**No issues found in #323 Raft scaffold (EXPERIMENTAL labelling already covers known gaps), #324 KMS, #329 Helm, #330 OTEL.**

Per the no-overclaim rule + Daniel's "Honest is better than fake" feedback: the verify-incompleteness on #327 is the kind of overclaim I should NOT have shipped without flagging — calling something "tamper-evidence" when it only checks chain linkage understates the gap. Future verify-style features will state explicitly which of `{linkage, hash, signature}` they verify.
