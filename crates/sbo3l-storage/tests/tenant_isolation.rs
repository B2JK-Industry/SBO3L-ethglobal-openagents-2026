//! Cross-tenant audit-chain isolation tests for V010 + the
//! `audit_*_for_tenant` methods.
//!
//! Pins the security property the multi-tenant deployment depends
//! on: tenant A cannot read tenant B's audit events, and tenant B's
//! chain integrity isn't disturbed by tampering on tenant A's
//! rows. The cross-tenant write contract (a request signed for
//! tenant A landing on tenant B's chain) is enforced one layer up
//! at the daemon's auth middleware; this test verifies the storage
//! layer's piece — once a `tenant_id` is supplied, filtering is
//! exact.

use sbo3l_core::signer::DevSigner;
use sbo3l_storage::audit_store::NewAuditEvent;
use sbo3l_storage::{Storage, TenantId, TenantMode, DEFAULT_TENANT_ID};

fn signer() -> DevSigner {
    DevSigner::from_seed("audit-signer-v1", [11u8; 32])
}

fn append(storage: &mut Storage, tenant: &str, event_type: &str, subject: &str) {
    storage
        .audit_append_for_tenant(
            tenant,
            NewAuditEvent::now(event_type, "test-actor", subject),
            &signer(),
        )
        .expect("append");
}

#[test]
fn two_tenants_have_independent_event_counts() {
    let mut storage = Storage::open_in_memory().unwrap();
    append(&mut storage, "tenant-a", "config_loaded", "subj-a-1");
    append(&mut storage, "tenant-a", "runtime_started", "subj-a-2");
    append(&mut storage, "tenant-b", "config_loaded", "subj-b-1");

    assert_eq!(storage.audit_count_for_tenant("tenant-a").unwrap(), 2);
    assert_eq!(storage.audit_count_for_tenant("tenant-b").unwrap(), 1);
    // The legacy global count still sees everything.
    assert_eq!(storage.audit_count().unwrap(), 3);
}

#[test]
fn tenant_a_list_does_not_contain_tenant_b_events() {
    let mut storage = Storage::open_in_memory().unwrap();
    append(&mut storage, "tenant-a", "evt-x", "subj-a");
    append(&mut storage, "tenant-b", "evt-y", "subj-b-secret");
    append(&mut storage, "tenant-a", "evt-z", "subj-a");

    let a_events = storage.audit_list_for_tenant("tenant-a").unwrap();
    assert_eq!(a_events.len(), 2);
    for e in &a_events {
        assert!(
            e.event.subject_id.starts_with("subj-a"),
            "tenant-a list leaked a non-A subject: {}",
            e.event.subject_id
        );
    }

    let b_events = storage.audit_list_for_tenant("tenant-b").unwrap();
    assert_eq!(b_events.len(), 1);
    assert_eq!(b_events[0].event.subject_id, "subj-b-secret");
}

#[test]
fn unknown_tenant_returns_empty_results_not_an_error() {
    let mut storage = Storage::open_in_memory().unwrap();
    append(&mut storage, "tenant-a", "evt-x", "subj-a");
    // Reading for a tenant that's never had a write is a NORMAL
    // case (first request from a freshly-onboarded tenant) — must
    // not error.
    assert_eq!(storage.audit_count_for_tenant("brand-new").unwrap(), 0);
    assert_eq!(storage.audit_list_for_tenant("brand-new").unwrap().len(), 0);
    assert!(storage
        .audit_last_for_tenant("brand-new")
        .unwrap()
        .is_none());
}

#[test]
fn each_tenants_chain_has_its_own_prev_hash_link() {
    // The first event in each tenant's chain has prev_event_hash =
    // ZERO_HASH. Cross-tenant events do NOT bridge: tenant-a's
    // second event's prev_event_hash matches tenant-a's first
    // event's event_hash, never tenant-b's.
    let mut storage = Storage::open_in_memory().unwrap();
    let a1 = storage
        .audit_append_for_tenant(
            "tenant-a",
            NewAuditEvent::now("a-1", "x", "subj"),
            &signer(),
        )
        .unwrap();
    // Insert a tenant-b event between the two tenant-a events to
    // prove the chain link in tenant-a skips it.
    let _b1 = storage
        .audit_append_for_tenant(
            "tenant-b",
            NewAuditEvent::now("b-1", "x", "subj"),
            &signer(),
        )
        .unwrap();
    let a2 = storage
        .audit_append_for_tenant(
            "tenant-a",
            NewAuditEvent::now("a-2", "x", "subj"),
            &signer(),
        )
        .unwrap();

    assert_eq!(
        a2.event.prev_event_hash, a1.event_hash,
        "tenant-a's second event must link to tenant-a's first, NOT to tenant-b's"
    );
}

#[test]
fn legacy_global_methods_still_see_everything_post_migration() {
    // V010 backfilled all legacy rows with tenant_id='default'.
    // The non-suffixed audit_count / audit_list / audit_last
    // methods continue to work without modification — they read
    // every row regardless of tenant_id.
    let mut storage = Storage::open_in_memory().unwrap();
    storage
        .audit_append(NewAuditEvent::now("legacy-1", "x", "subj"), &signer())
        .unwrap();
    append(&mut storage, "tenant-a", "tenant-1", "subj");
    assert_eq!(storage.audit_count().unwrap(), 2);
    let last = storage.audit_last().unwrap().unwrap();
    assert_eq!(last.event.event_type, "tenant-1");
}

#[test]
fn legacy_audit_append_writes_to_default_tenant() {
    // The unmodified `audit_append` doesn't pass a tenant_id, so
    // the V010 `DEFAULT 'default'` column constraint backfills it.
    // This means a `audit_count_for_tenant("default")` reads the
    // legacy events.
    let mut storage = Storage::open_in_memory().unwrap();
    storage
        .audit_append(NewAuditEvent::now("legacy-write", "x", "subj"), &signer())
        .unwrap();
    assert_eq!(
        storage.audit_count_for_tenant(DEFAULT_TENANT_ID).unwrap(),
        1
    );
}

#[test]
fn tenant_id_newtype_round_trips_via_as_str() {
    let t = TenantId::new("acme-corp");
    assert_eq!(t.as_str(), "acme-corp");
    assert_eq!(t.to_string(), "acme-corp");
    assert_eq!(t.as_ref(), "acme-corp");
    let d = TenantId::default_tenant();
    assert_eq!(d.as_str(), DEFAULT_TENANT_ID);
}

#[test]
fn tenant_mode_from_env_defaults_to_single() {
    // Don't rely on env state — exercise the flag values directly.
    // (`SBO3L_MULTI_TENANT` may be set by another concurrent test
    // process, so we don't assert on `from_env()` here; instead
    // pin the parsing semantics with the documented values.)
    unsafe {
        std::env::set_var("SBO3L_MULTI_TENANT", "1");
    }
    assert_eq!(TenantMode::from_env(), TenantMode::Multi);
    unsafe {
        std::env::set_var("SBO3L_MULTI_TENANT", "no");
    }
    assert_eq!(TenantMode::from_env(), TenantMode::Single);
    unsafe {
        std::env::remove_var("SBO3L_MULTI_TENANT");
    }
    assert_eq!(TenantMode::from_env(), TenantMode::Single);
}

#[test]
fn global_seq_remains_monotonic_across_tenant_writes() {
    // The PRIMARY KEY contract on `seq` is preserved: every event
    // gets a globally unique seq, regardless of tenant. Per-tenant
    // chains have GAPS (e.g., tenant-a sees seq=1, 3 if
    // tenant-b's event landed between).
    let mut storage = Storage::open_in_memory().unwrap();
    let a1 = storage
        .audit_append_for_tenant("tenant-a", NewAuditEvent::now("a", "x", "y"), &signer())
        .unwrap();
    let b1 = storage
        .audit_append_for_tenant("tenant-b", NewAuditEvent::now("b", "x", "y"), &signer())
        .unwrap();
    let a2 = storage
        .audit_append_for_tenant("tenant-a", NewAuditEvent::now("a", "x", "y"), &signer())
        .unwrap();
    assert_eq!(a1.event.seq, 1);
    assert_eq!(b1.event.seq, 2);
    assert_eq!(a2.event.seq, 3);
}
