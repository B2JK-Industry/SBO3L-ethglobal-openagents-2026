//! Integration tests for `/v1/admin/flags` GET + POST.
//!
//! Single multi-step test rather than N parallel tokio::tests —
//! the admin auth gate reads a process-global env var
//! (ADMIN_BEARER_HASH_ENV), so parallel tests would race on the
//! var's set/unset state. A sequential walk through the full
//! lifecycle (install → 401 paths → 503 path → 200 paths → flip)
//! exercises every contract without flake risk.
//!
//! bcrypt hashing is intentionally slow (~100ms+); we use a low
//! cost (4) for tests. Production bumps cost to 12+.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use sbo3l_server::feature_flags::ADMIN_BEARER_HASH_ENV;
use sbo3l_server::{reference_policy, router, AppState};
use sbo3l_storage::Storage;
use serde_json::{json, Value};
use tower::ServiceExt;

const ADMIN_SECRET: &str = "admin-test-secret-please-do-not-leak-in-prod";

fn install_admin_hash() {
    let hash = bcrypt::hash(ADMIN_SECRET, 4).expect("bcrypt hash");
    unsafe {
        std::env::set_var(ADMIN_BEARER_HASH_ENV, hash);
    }
}
fn uninstall_admin_hash() {
    unsafe {
        std::env::remove_var(ADMIN_BEARER_HASH_ENV);
    }
}

fn build_app() -> axum::Router {
    let storage = Storage::open_in_memory().unwrap();
    router(AppState::new(reference_policy(), storage))
}

fn admin_get(token: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("GET").uri("/v1/admin/flags");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

fn admin_post(token: Option<&str>, body: Value) -> Request<Body> {
    let mut b = Request::builder()
        .method("POST")
        .uri("/v1/admin/flags")
        .header("content-type", "application/json");
    if let Some(t) = token {
        b = b.header("authorization", format!("Bearer {t}"));
    }
    b.body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

async fn read_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

#[tokio::test]
async fn admin_flags_lifecycle() {
    // ---- 503 path: env var not set ----
    uninstall_admin_hash();
    let resp = build_app()
        .oneshot(admin_get(Some("anything")))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::SERVICE_UNAVAILABLE,
        "no admin credential configured must surface 503"
    );
    let body = read_body(resp).await;
    assert_eq!(body["code"], "admin.no_credential_configured");

    // ---- install hash for the rest of the lifecycle ----
    install_admin_hash();

    // ---- 401: missing token ----
    let resp = build_app().oneshot(admin_get(None)).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "admin.missing_token");

    // ---- 401: wrong token ----
    let resp = build_app()
        .oneshot(admin_get(Some("not-the-admin-secret")))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "admin.invalid_token");

    // ---- 200: GET on fresh daemon → empty array ----
    let resp = build_app()
        .oneshot(admin_get(Some(ADMIN_SECRET)))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;
    assert_eq!(body["schema"], "sbo3l.feature_flags_list.v1");
    assert_eq!(body["flags"].as_array().unwrap().len(), 0);

    // ---- POST: set flag.demo=true, verify response shape + audit id ----
    let app = build_app();
    let resp = app
        .clone()
        .oneshot(admin_post(
            Some(ADMIN_SECRET),
            json!({ "key": "flag.demo", "enabled": true }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = read_body(resp).await;
    assert_eq!(body["schema"], "sbo3l.feature_flag_set.v1");
    assert_eq!(body["flag"]["key"], "flag.demo");
    assert_eq!(body["flag"]["enabled"], true);
    assert_eq!(body["flag"]["last_actor"], "admin");
    let audit_id = body["audit_event_id"]
        .as_str()
        .expect("audit_event_id present");
    assert!(
        audit_id.starts_with("evt-"),
        "audit_event_id must start with evt-, got {audit_id}"
    );

    // ---- GET reflects the new flag ----
    let resp = app
        .clone()
        .oneshot(admin_get(Some(ADMIN_SECRET)))
        .await
        .unwrap();
    let body = read_body(resp).await;
    let flags = body["flags"].as_array().unwrap();
    assert_eq!(flags.len(), 1);
    assert_eq!(flags[0]["key"], "flag.demo");
    assert_eq!(flags[0]["enabled"], true);

    // ---- POST flip flag.demo=false → reflected in GET ----
    app.clone()
        .oneshot(admin_post(
            Some(ADMIN_SECRET),
            json!({ "key": "flag.demo", "enabled": false }),
        ))
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(admin_get(Some(ADMIN_SECRET)))
        .await
        .unwrap();
    let body = read_body(resp).await;
    let demo = body["flags"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["key"] == "flag.demo")
        .expect("flag.demo present");
    assert_eq!(demo["enabled"], false);

    // ---- empty key rejected with 400 ----
    let resp = app
        .clone()
        .oneshot(admin_post(
            Some(ADMIN_SECRET),
            json!({ "key": "", "enabled": true }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body = read_body(resp).await;
    assert_eq!(body["code"], "admin.bad_key");

    // ---- unknown field rejected (deny_unknown_fields on SetFlagRequest) ----
    let resp = app
        .clone()
        .oneshot(admin_post(
            Some(ADMIN_SECRET),
            json!({
                "key": "flag.x",
                "enabled": true,
                "rogue_extra": "should be rejected"
            }),
        ))
        .await
        .unwrap();
    assert!(
        resp.status().is_client_error(),
        "expected client error on unknown field, got {}",
        resp.status()
    );

    // ---- POST 3 flags, GET returns them sorted by key ----
    for k in ["flag.zebra", "flag.alpha", "flag.mango"] {
        app.clone()
            .oneshot(admin_post(
                Some(ADMIN_SECRET),
                json!({ "key": k, "enabled": true }),
            ))
            .await
            .unwrap();
    }
    let resp = app.oneshot(admin_get(Some(ADMIN_SECRET))).await.unwrap();
    let body = read_body(resp).await;
    let keys: Vec<&str> = body["flags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["key"].as_str().unwrap())
        .collect();
    // flag.demo is also present from earlier — assert sort order
    // includes the new entries in the right spot.
    assert!(keys.contains(&"flag.alpha"));
    assert!(keys.contains(&"flag.mango"));
    assert!(keys.contains(&"flag.zebra"));
    let alpha_idx = keys.iter().position(|k| *k == "flag.alpha").unwrap();
    let mango_idx = keys.iter().position(|k| *k == "flag.mango").unwrap();
    let zebra_idx = keys.iter().position(|k| *k == "flag.zebra").unwrap();
    assert!(
        alpha_idx < mango_idx && mango_idx < zebra_idx,
        "GET must return flags sorted by key; got {keys:?}"
    );

    uninstall_admin_hash();
}
