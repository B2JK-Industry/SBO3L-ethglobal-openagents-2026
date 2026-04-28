//! `mandate doctor` — operator readiness summary.
//!
//! Production-shaped, deliberately honest. Every feature shows up as one
//! of:
//!
//! - **`ok`** — the feature is implemented and the database backs it.
//! - **`skip`** — the feature is not implemented in this build (its table
//!   doesn't exist yet, or its CLI/storage path is documented future work).
//!   Skip is **never** silently presented as `ok`.
//! - **`warn`** — implemented but the current state is anomalous (e.g. an
//!   empty audit chain on a fresh DB is fine; an empty audit chain on a
//!   db that has nonce rows is suspicious — but we don't currently
//!   correlate those, so warn is reserved for future heuristics).
//! - **`fail`** — implemented but the integrity check failed (e.g. audit
//!   chain present but `prev_event_hash` linkage is broken). The doctor
//!   exits non-zero in this case.
//!
//! The CLI is intended to be safe to run against a production-shape mock
//! daemon's storage. It does not require live network, live KMS or
//! sponsor credentials — every check is a local SQLite read.

use std::path::Path;
use std::process::ExitCode;

use mandate_core::audit::verify_chain;
use mandate_storage::Storage;
use serde::Serialize;

/// One row of the doctor report. Either a present-and-healthy `ok` row, a
/// `skip` row for an optional/future feature, a `warn`, or a `fail`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum CheckStatus {
    Ok {
        detail: String,
    },
    Skip {
        reason: String,
    },
    /// Reserved for future heuristics (see module docs). Currently no
    /// runtime check produces a `Warn`; it stays in the enum so the
    /// JSON envelope is forward-compatible and tests can construct it
    /// to exercise `overall_verdict`.
    #[allow(dead_code)]
    Warn {
        detail: String,
    },
    Fail {
        error: String,
    },
}

#[derive(Debug, Clone, Serialize)]
struct Check {
    name: &'static str,
    #[serde(flatten)]
    status: CheckStatus,
}

/// JSON envelope shape — stable across releases. The production-shaped
/// runner (PSM-B1) can consume this to promote sections from SKIP.
#[derive(Debug, Serialize)]
struct DoctorReport {
    /// Always `"mandate.doctor.v1"`. Stable identifier so consumers can
    /// route to the right parser.
    report_type: &'static str,
    /// Aggregate verdict. `ok` if every check is `ok` or `skip`. `fail`
    /// if any check is `fail`. `warn` if there's at least one `warn` and
    /// no `fail`.
    overall: &'static str,
    /// Path the doctor opened. `:memory:` if `--db` was omitted.
    db_path: String,
    checks: Vec<Check>,
}

fn ok(name: &'static str, detail: impl Into<String>) -> Check {
    Check {
        name,
        status: CheckStatus::Ok {
            detail: detail.into(),
        },
    }
}

fn skip(name: &'static str, reason: impl Into<String>) -> Check {
    Check {
        name,
        status: CheckStatus::Skip {
            reason: reason.into(),
        },
    }
}

/// Constructor for warn rows. Reserved for future heuristics — exposed
/// to tests so `overall_verdict` is exercised, but no runtime check
/// produces a warn today.
#[allow(dead_code)]
fn warn_check(name: &'static str, detail: impl Into<String>) -> Check {
    Check {
        name,
        status: CheckStatus::Warn {
            detail: detail.into(),
        },
    }
}

fn fail(name: &'static str, error: impl Into<String>) -> Check {
    Check {
        name,
        status: CheckStatus::Fail {
            error: error.into(),
        },
    }
}

/// Run every check against `storage` and return the populated report.
fn run_checks(storage: &Storage) -> Vec<Check> {
    let mut checks = Vec::new();

    // 1. migrations applied
    match storage.applied_migrations() {
        Ok(rows) if rows.is_empty() => {
            checks.push(fail("migrations", "no migrations recorded"));
        }
        Ok(rows) => {
            let summary = rows
                .iter()
                .map(|(v, d)| format!("V{v:03}:{d}"))
                .collect::<Vec<_>>()
                .join(", ");
            checks.push(ok("migrations", summary));
        }
        Err(e) => checks.push(fail("migrations", e.to_string())),
    }

    // 2. nonce replay table (V002 — production-grade in current build)
    match storage.optional_count("nonce_replay") {
        Ok(Some(n)) => checks.push(ok("nonce_replay", format!("table present, rows={n}"))),
        Ok(None) => checks.push(skip(
            "nonce_replay",
            "table not present — nonce replay protection unavailable in this DB",
        )),
        Err(e) => checks.push(fail("nonce_replay", e.to_string())),
    }

    // 3. idempotency table (V004 — present after PSM-A2 lands)
    match storage.optional_count("idempotency_keys") {
        Ok(Some(n)) => checks.push(ok("idempotency_keys", format!("table present, rows={n}"))),
        Ok(None) => checks.push(skip(
            "idempotency_keys",
            "table not present — Idempotency-Key safe-retry unavailable in this DB \
             (implemented in PSM-A2; ensure the daemon ran the V004 migration)",
        )),
        Err(e) => checks.push(fail("idempotency_keys", e.to_string())),
    }

    // 4. audit chain — present, structural verify if non-empty
    match storage.audit_count() {
        Ok(0) => checks.push(skip(
            "audit_chain",
            "no audit events yet — fresh DB; doctor cannot verify a chain that hasn't been written",
        )),
        Ok(n) => match storage.audit_list() {
            Ok(events) => match verify_chain(&events, true, None) {
                Ok(()) => checks.push(ok(
                    "audit_chain",
                    format!("{n} events; structural + hash verify ok (signatures not checked — pubkey not available to doctor)"),
                )),
                Err(e) => checks.push(fail("audit_chain", e.to_string())),
            },
            Err(e) => checks.push(fail("audit_chain", e.to_string())),
        },
        Err(e) => checks.push(fail("audit_chain", e.to_string())),
    }

    // 5. mock KMS keyring (PSM-A1 / future PSM-A1.9 storage)
    match storage.optional_count("mock_kms_keys") {
        Ok(Some(n)) => checks.push(ok(
            "mock_kms_keys",
            format!(
                "table present, rows={n} — see docs/cli/mock-kms.md (mock, not production KMS)"
            ),
        )),
        Ok(None) => checks.push(skip(
            "mock_kms_keys",
            "table not present — Mock KMS keyring persistence is tracked as PSM-A1.9; \
             the in-process MockKmsSigner from PSM-A1 still works without it",
        )),
        Err(e) => checks.push(fail("mock_kms_keys", e.to_string())),
    }

    // 6. active policy (PSM-A3)
    //
    // Three states, all honest:
    //   - table present + an active row: `ok` with version + 12-char
    //     hash prefix (full hash in `mandate policy current`).
    //   - table present + no active row: `skip` ("table is here, no
    //     policy seeded yet — run `mandate policy activate <file>`").
    //   - table missing entirely: `skip` ("older daemon DB before V006").
    match storage.optional_count("active_policy") {
        Ok(Some(_)) => match storage.policy_current() {
            Ok(Some(rec)) => {
                let prefix: String = rec.policy_hash.chars().take(12).collect();
                let total = storage.policy_list().map(|v| v.len()).unwrap_or(0);
                checks.push(ok(
                    "active_policy",
                    format!(
                        "table present, rows={total}, active=v{ver}, hash={prefix}…",
                        ver = rec.version,
                    ),
                ));
            }
            Ok(None) => checks.push(skip(
                "active_policy",
                "table present but no policy activated yet — run \
                 `mandate policy activate <file> --db <path>` to seed one (PSM-A3)",
            )),
            Err(e) => checks.push(fail("active_policy", e.to_string())),
        },
        Ok(None) => checks.push(skip(
            "active_policy",
            "table not present — older daemon DB before V006; the daemon \
             still uses the embedded reference policy until `mandate policy \
             activate` runs against an upgraded DB (PSM-A3)",
        )),
        Err(e) => checks.push(fail("active_policy", e.to_string())),
    }

    // 7. payment_requests (existing table from V001)
    match storage.optional_count("payment_requests") {
        Ok(Some(n)) => checks.push(ok("payment_requests", format!("table present, rows={n}"))),
        Ok(None) => checks.push(fail(
            "payment_requests",
            "core table missing — V001 migration did not apply",
        )),
        Err(e) => checks.push(fail("payment_requests", e.to_string())),
    }

    checks
}

fn overall_verdict(checks: &[Check]) -> &'static str {
    let mut has_warn = false;
    for c in checks {
        match c.status {
            CheckStatus::Fail { .. } => return "fail",
            CheckStatus::Warn { .. } => has_warn = true,
            _ => {}
        }
    }
    if has_warn {
        "warn"
    } else {
        "ok"
    }
}

fn render_human(report: &DoctorReport) -> String {
    let mut out = String::new();
    use std::fmt::Write;
    let _ = writeln!(out, "mandate doctor — operator readiness summary");
    let _ = writeln!(out, "  db:      {}", report.db_path);
    let _ = writeln!(out, "  overall: {}", report.overall);
    let _ = writeln!(out);
    for c in &report.checks {
        match &c.status {
            CheckStatus::Ok { detail } => {
                let _ = writeln!(out, "  ok    {:24}  {}", c.name, detail);
            }
            CheckStatus::Skip { reason } => {
                let _ = writeln!(out, "  skip  {:24}  {}", c.name, reason);
            }
            CheckStatus::Warn { detail } => {
                let _ = writeln!(out, "  warn  {:24}  {}", c.name, detail);
            }
            CheckStatus::Fail { error } => {
                let _ = writeln!(out, "  FAIL  {:24}  {}", c.name, error);
            }
        }
    }
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "  truthfulness note: skip means the feature is not yet implemented \
         in this build, NOT that it silently passed. See docs/cli/doctor.md."
    );
    out
}

/// Entry point for `mandate doctor`. Returns a process exit code:
/// `0` on `ok` / `warn`, `1` if any check failed, `2` if the database
/// itself could not be opened.
pub fn run(db: Option<&Path>, json: bool) -> ExitCode {
    // doctor must be inspection-only. `Storage::open` (rusqlite default)
    // would create a fresh SQLite file at `--db` if it didn't exist and
    // run migrations against it — which silently mutates the operator's
    // filesystem and produces a misleading "ok" report for a DB that
    // never existed. Pre-check existence and refuse to open a missing
    // path. Codex P1 review on PR #25.
    //
    // Codex P2 follow-up on PR #32 asked us to use `try_exists()` instead
    // of `exists()`: `exists()` swallows fs/permission errors as `false`,
    // which would mis-report a permission-denied path as "does not exist"
    // and exit 2. With `try_exists()`:
    //   - Ok(false) → file genuinely missing → exit 2 with "does not exist"
    //   - Ok(true)  → file present → fall through to `Storage::open`
    //   - Err(e)    → fs/permission metadata error → fall through to
    //                 `Storage::open`, which surfaces the real cause as
    //                 a `storage_open` fail (the existing error path).
    if let Some(p) = db {
        match p.try_exists() {
            Ok(false) => {
                let path = p.display().to_string();
                let msg = format!("doctor target DB does not exist: {path}");
                if json {
                    let report = DoctorReport {
                        report_type: "mandate.doctor.v1",
                        overall: "fail",
                        db_path: path,
                        checks: vec![fail("storage_open", msg)],
                    };
                    println!("{}", serde_json::to_string_pretty(&report).unwrap());
                } else {
                    eprintln!("mandate doctor: {msg}");
                }
                return ExitCode::from(2);
            }
            Ok(true) => {
                // file is there; let Storage::open validate it
            }
            Err(_) => {
                // metadata error (e.g. permission denied on parent
                // directory). Don't claim "does not exist"; let the
                // existing storage_open fail path surface the real
                // OS-level error verbatim.
            }
        }
    }

    let storage_result = match db {
        Some(p) => Storage::open(p),
        None => Storage::open_in_memory(),
    };
    let storage = match storage_result {
        Ok(s) => s,
        Err(e) => {
            let path = db
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ":memory:".to_string());
            if json {
                let report = DoctorReport {
                    report_type: "mandate.doctor.v1",
                    overall: "fail",
                    db_path: path.clone(),
                    checks: vec![fail("storage_open", e.to_string())],
                };
                println!("{}", serde_json::to_string_pretty(&report).unwrap());
            } else {
                eprintln!("mandate doctor: failed to open db {path}: {e}");
            }
            return ExitCode::from(2);
        }
    };

    let checks = run_checks(&storage);
    let overall = overall_verdict(&checks);
    let report = DoctorReport {
        report_type: "mandate.doctor.v1",
        overall,
        db_path: db
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ":memory:".to_string()),
        checks,
    };

    if json {
        match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("mandate doctor: failed to serialise report: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        print!("{}", render_human(&report));
    }

    match overall {
        "fail" => ExitCode::from(1),
        _ => ExitCode::SUCCESS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_storage() -> Storage {
        Storage::open_in_memory().unwrap()
    }

    /// Build a tempfile-backed `Storage` and then DROP `table` from
    /// the underlying SQLite file, simulating an "older daemon DB"
    /// that was migrated to current schema except for the named
    /// optional table. We use this to test the doctor's `skip` path
    /// independently of which migrations happen to be on `main` at
    /// any given moment — so the test stays correct after future
    /// migrations land.
    ///
    /// The migration system records `(version, sha256)` rows in
    /// `schema_migrations`. After we drop the table, re-opening the
    /// `Storage` does NOT re-create it: the migration loop sees the
    /// existing `schema_migrations` row, skips re-applying. That
    /// precisely models a daemon that was upgraded without re-running
    /// the relevant migration — exactly the operational shape the
    /// doctor's skip path is designed to surface.
    fn storage_without_table(table: &str) -> (tempfile::TempDir, Storage) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("doctor-older-db.sqlite");
        // Apply all current migrations (V001/V002/V004 today, plus
        // anything future). Drop the Storage so the file lock is
        // released before the raw rusqlite handle takes over.
        {
            let _ = Storage::open(&path).unwrap();
        }
        {
            let conn = rusqlite::Connection::open(&path).unwrap();
            conn.execute(&format!("DROP TABLE IF EXISTS \"{table}\""), [])
                .unwrap();
        }
        // Re-open via Storage. Migration loop sees `schema_migrations`
        // already populated for this version → skips re-applying.
        // The dropped table stays missing.
        let s = Storage::open(&path).unwrap();
        (tmp, s)
    }

    #[test]
    fn fresh_in_memory_db_yields_ok_overall() {
        // A fresh storage on current `main` has:
        //   - migrations applied (V001 + V002 + V004 + V005 + V006),
        //   - nonce_replay (V002), idempotency_keys (V004),
        //     mock_kms_keys (V005), payment_requests (V001) — all
        //     present, all rows=0 → ok rows.
        //   - active_policy (V006) table present but no policy seeded
        //     yet → skip with PSM-A3 pointer (truthfulness rule:
        //     "table is here but you haven't activated anything yet"
        //     is honest, not fake-ok).
        //   - audit_chain → skip (no events yet).
        // No fails, no warns → overall ok.
        let s = fresh_storage();
        let checks = run_checks(&s);
        let overall = overall_verdict(&checks);
        assert_eq!(overall, "ok", "got checks={checks:#?}");
        // Sanity: at least one ok and at least one skip should be present.
        let has_ok = checks
            .iter()
            .any(|c| matches!(c.status, CheckStatus::Ok { .. }));
        let has_skip = checks
            .iter()
            .any(|c| matches!(c.status, CheckStatus::Skip { .. }));
        assert!(has_ok, "doctor must surface at least one ok check");
        assert!(
            has_skip,
            "doctor must surface skip rows for optional features (truthfulness rule)"
        );
    }

    #[test]
    fn nonce_replay_reports_ok_when_table_present() {
        let s = fresh_storage();
        let checks = run_checks(&s);
        let nonce_check = checks
            .iter()
            .find(|c| c.name == "nonce_replay")
            .expect("nonce_replay must be checked");
        assert!(matches!(nonce_check.status, CheckStatus::Ok { .. }));
    }

    #[test]
    fn idempotency_reports_ok_on_default_post_a2_db() {
        // Post PSM-A2, V004 is part of the default migration set, so a
        // fresh storage carries the `idempotency_keys` table. This
        // pins the OK path so a future regression that silently
        // dropped V004 from the migrations list would surface here.
        let s = fresh_storage();
        let checks = run_checks(&s);
        let idem = checks
            .iter()
            .find(|c| c.name == "idempotency_keys")
            .expect("idempotency_keys row must exist");
        assert!(
            matches!(idem.status, CheckStatus::Ok { .. }),
            "idempotency_keys must be OK on a default post-A2 DB; got {:?}",
            idem.status
        );
    }

    #[test]
    fn idempotency_skip_when_table_missing_on_older_db() {
        // The skip path is ONLY for older databases that haven't run
        // the V004 migration (e.g. a daemon that was upgraded but
        // hasn't applied schema changes yet). We construct that shape
        // explicitly by dropping the `idempotency_keys` table —
        // independent of which migrations happen to be on main. The
        // skip reason must reference PSM-A2 so an operator knows the
        // upgrade path.
        let (_tmp, s) = storage_without_table("idempotency_keys");
        let checks = run_checks(&s);
        let idem = checks
            .iter()
            .find(|c| c.name == "idempotency_keys")
            .expect("idempotency_keys row must exist");
        match &idem.status {
            CheckStatus::Skip { reason } => {
                assert!(
                    reason.contains("PSM-A2"),
                    "skip reason must reference PSM-A2 so operators know what to enable: {reason}"
                );
            }
            other => panic!("expected skip on older DB without V004, got {other:?}"),
        }
    }

    #[test]
    fn active_policy_skip_when_no_policy_seeded() {
        // PSM-A3 truthfulness rule: a fresh DB has the V006 table but
        // nothing has been activated yet. Doctor must surface this as
        // skip with a pointer at `mandate policy activate`, not as
        // ok or as a missing-table skip — the table IS here, it's the
        // policy that's missing.
        let s = fresh_storage();
        let checks = run_checks(&s);
        let row = checks
            .iter()
            .find(|c| c.name == "active_policy")
            .expect("active_policy row");
        match &row.status {
            CheckStatus::Skip { reason } => {
                assert!(
                    reason.contains("activate") && reason.contains("PSM-A3"),
                    "skip reason must point at `mandate policy activate` and PSM-A3; got: {reason}"
                );
            }
            other => panic!("expected skip on freshly migrated DB without policy, got {other:?}"),
        }
    }

    #[test]
    fn active_policy_reports_ok_after_activate() {
        // After a policy is activated, the row flips to ok and
        // surfaces both the version and a 12-char hash prefix so an
        // operator can confirm which policy is live. We hand the
        // storage layer an arbitrary stable JSON + hash here — the
        // doctor only reads what's persisted, not what `Policy::parse`
        // would have produced (that's covered by the CLI integration
        // test that pins the embedded reference hash).
        use chrono::Utc;
        let mut s = fresh_storage();
        let policy_json = r#"{"version":1,"agents":[],"rules":[{"id":"r-1"}],"providers":[],"recipients":[],"budgets":[]}"#;
        let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        s.policy_activate(policy_json, hash, "operator-cli", Utc::now())
            .unwrap();
        let checks = run_checks(&s);
        let row = checks
            .iter()
            .find(|c| c.name == "active_policy")
            .expect("active_policy row");
        match &row.status {
            CheckStatus::Ok { detail } => {
                assert!(detail.contains("active=v1"), "got: {detail}");
                let prefix = &hash[..12];
                assert!(
                    detail.contains(prefix),
                    "detail must include the first 12 hex chars of the hash; got: {detail}"
                );
            }
            other => panic!("expected ok after activate, got {other:?}"),
        }
    }

    #[test]
    fn active_policy_skip_when_table_missing_on_older_db() {
        // Pre-V006 daemon DB: drop the table to simulate an older
        // daemon that hasn't applied V006 yet. The skip must point
        // operators at the V006 migration / `mandate policy activate`
        // flow.
        let (_tmp, s) = storage_without_table("active_policy");
        let checks = run_checks(&s);
        let row = checks
            .iter()
            .find(|c| c.name == "active_policy")
            .expect("active_policy row");
        match &row.status {
            CheckStatus::Skip { reason } => {
                assert!(
                    reason.contains("PSM-A3") && reason.contains("V006"),
                    "skip reason must reference PSM-A3 + V006: {reason}"
                );
            }
            other => panic!("expected skip on older DB without V006, got {other:?}"),
        }
    }

    #[test]
    fn mock_kms_keys_skip_when_table_missing_on_older_db() {
        // Same pattern as the idempotency skip test but for V005
        // (PSM-A1.9). On main today the table doesn't exist by
        // default, but once A1.9 lands it will — so we explicitly
        // construct the skip shape via the table-drop helper. That
        // keeps this test correct in BOTH worlds: pre-A1.9 the table
        // is naturally absent, post-A1.9 we drop it to simulate an
        // older daemon DB. The skip reason must reference PSM-A1.9
        // so an operator knows the upgrade path.
        let (_tmp, s) = storage_without_table("mock_kms_keys");
        let checks = run_checks(&s);
        let kms = checks
            .iter()
            .find(|c| c.name == "mock_kms_keys")
            .expect("mock_kms_keys row");
        match &kms.status {
            CheckStatus::Skip { reason } => {
                assert!(
                    reason.contains("PSM-A1.9"),
                    "skip reason must reference PSM-A1.9: {reason}"
                );
            }
            other => panic!("expected skip, got {other:?}"),
        }
    }

    #[test]
    fn audit_chain_skip_on_empty_db() {
        let s = fresh_storage();
        let checks = run_checks(&s);
        let chain = checks
            .iter()
            .find(|c| c.name == "audit_chain")
            .expect("audit_chain row");
        assert!(
            matches!(chain.status, CheckStatus::Skip { .. }),
            "empty chain must be skip, not fail or fake-ok; got {:?}",
            chain.status
        );
    }

    #[test]
    fn audit_chain_ok_after_appending_events() {
        // Append a couple of events through the existing storage path,
        // then run the doctor — chain row should be ok with the count.
        use mandate_core::signer::DevSigner;
        use mandate_storage::audit_store::NewAuditEvent;
        let mut s = fresh_storage();
        let signer = DevSigner::from_seed("audit-doctor", [3u8; 32]);
        s.audit_append(
            NewAuditEvent::now("runtime_started", "doctor-test", "runtime"),
            &signer,
        )
        .unwrap();
        s.audit_append(
            NewAuditEvent::now("policy_decided", "doctor-test", "pr-001"),
            &signer,
        )
        .unwrap();
        let checks = run_checks(&s);
        let chain = checks.iter().find(|c| c.name == "audit_chain").unwrap();
        match &chain.status {
            CheckStatus::Ok { detail } => assert!(
                detail.contains("2 events"),
                "expected ok with 2 events, got {detail}"
            ),
            other => panic!("expected ok, got {other:?}"),
        }
    }

    #[test]
    fn json_envelope_serialises_with_stable_report_type() {
        let s = fresh_storage();
        let checks = run_checks(&s);
        let overall = overall_verdict(&checks);
        let report = DoctorReport {
            report_type: "mandate.doctor.v1",
            overall,
            db_path: ":memory:".to_string(),
            checks,
        };
        let v: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&report).unwrap()).unwrap();
        assert_eq!(v["report_type"], "mandate.doctor.v1");
        assert!(v["checks"].is_array());
        assert!(v["checks"].as_array().unwrap().len() >= 5);
        // Each check carries a name + a status tag.
        for c in v["checks"].as_array().unwrap() {
            assert!(c["name"].is_string());
            assert!(c["status"].is_string());
        }
    }

    #[test]
    fn overall_is_fail_if_any_check_fails() {
        let checks = vec![
            ok("a", "fine"),
            fail("b", "broken"),
            skip("c", "not implemented"),
        ];
        assert_eq!(overall_verdict(&checks), "fail");
    }

    #[test]
    fn overall_is_warn_if_only_warns_no_fails() {
        let checks = vec![
            ok("a", "fine"),
            warn_check("b", "anomalous"),
            skip("c", "not implemented"),
        ];
        assert_eq!(overall_verdict(&checks), "warn");
    }

    #[test]
    fn overall_is_ok_if_only_ok_and_skip() {
        let checks = vec![ok("a", "fine"), skip("b", "not implemented")];
        assert_eq!(overall_verdict(&checks), "ok");
    }
}
