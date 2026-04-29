//! ETHGlobal Open Agents demo harness.
//!
//! Drives SBO3L's payment-request pipeline end-to-end against the
//! in-memory daemon. Output is deterministic — no LLM credentials, no
//! network calls.
//!
//! Three demo modes:
//!
//! * `--scenario legit-x402 | prompt-injection` — load the named APRP
//!   fixture from `scenarios.json`, post it to SBO3L, print decision +
//!   receipt + audit event id. Optional `--ens-fixture` /
//!   `--execute-keeperhub`.
//! * `--uniswap-quote <quote.json> --swap-policy <swap-policy.json>` —
//!   run the swap-policy guard, build an APRP from the quote, post it to
//!   SBO3L (using `--policy` if provided), print swap-policy outcome +
//!   SBO3L decision. Optional `--execute-uniswap`.
//! * `--policy <policy.json>` — override the bundled reference policy for
//!   any of the modes above.

use std::process::ExitCode;

use chrono::{Duration, Utc};
use clap::Parser;
use serde::Deserialize;
use serde_json::Value;

use sbo3l_execution::uniswap::{evaluate_swap, SwapPolicy, SwapQuote};
use sbo3l_execution::{GuardedExecutor, KeeperHubExecutor, SwapPolicyOutcome, UniswapExecutor};
use sbo3l_identity::{EnsResolver, OfflineEnsResolver};
use sbo3l_policy::Policy;
use sbo3l_server::{reference_policy, AppState, PaymentRequestResponse};
use sbo3l_storage::Storage;

#[derive(Parser, Debug)]
#[command(
    name = "research-agent",
    version,
    about = "SBO3L ETHGlobal Open Agents research-agent harness."
)]
struct Cli {
    /// Scenario id from scenarios.json: legit-x402 | prompt-injection. Mutually
    /// exclusive with `--uniswap-quote`.
    #[arg(long)]
    scenario: Option<String>,
    /// Path to scenarios.json (defaults to next to the binary)
    #[arg(long, default_value = "demo-agents/research-agent/scenarios.json")]
    scenarios: std::path::PathBuf,

    /// Override the bundled reference SBO3L policy. Used by the Uniswap
    /// demo to load the swap-aware policy variant in
    /// `demo-fixtures/uniswap/sbo3l-policy.json`.
    #[arg(long)]
    policy: Option<std::path::PathBuf>,

    /// Optional ENS records fixture (`{"name.eth": {...}}`). When set, resolve
    /// `--ens-name` and verify the `sbo3l:policy_hash` text record matches
    /// the canonical hash of the active reference policy.
    #[arg(long)]
    ens_fixture: Option<std::path::PathBuf>,
    /// ENS name to resolve (used with --ens-fixture).
    #[arg(long, default_value = "research-agent.team.eth")]
    ens_name: String,

    /// After an `allow` decision, route the action through the KeeperHub
    /// guarded-execution adapter (local mock).
    #[arg(long, default_value_t = false)]
    execute_keeperhub: bool,

    /// After an `allow` decision, route the action through the Uniswap
    /// guarded-execution adapter (local mock).
    #[arg(long, default_value_t = false)]
    execute_uniswap: bool,

    /// Path to a Uniswap quote fixture (see demo-fixtures/uniswap/). When set,
    /// the harness runs the swap-policy guard and then submits the quote as
    /// an APRP `smart_account_session` request to SBO3L.
    #[arg(long)]
    uniswap_quote: Option<std::path::PathBuf>,
    /// Path to the swap-policy fixture used by the swap-policy guard.
    #[arg(long, default_value = "demo-fixtures/uniswap/swap-policy.json")]
    swap_policy: std::path::PathBuf,
    /// Relax the freshness check on a static fixture clock (the demo's
    /// fixture uses a fixed `fetched_at_unix`). Live mode applies the strict
    /// check.
    #[arg(long, default_value_t = true)]
    relax_quote_freshness: bool,

    /// Optional path to a persistent SQLite storage file for the in-process
    /// daemon. When unset, the harness uses `Storage::open_in_memory()` (the
    /// default for the existing 13-gate demo). When set, the same migrations
    /// run against the on-disk file, the chain persists past the harness
    /// process, and the file is suitable for `sbo3l audit export --db`.
    /// Used by `demo-scripts/run-production-shaped-mock.sh`.
    #[arg(long)]
    storage_path: Option<std::path::PathBuf>,

    /// Optional path to write the signed `PolicyReceipt` JSON returned by
    /// the daemon. Lets downstream scripts (e.g. the production-shaped mock
    /// runner) feed the receipt into `sbo3l audit export --receipt`
    /// without parsing the harness's human-readable stdout.
    #[arg(long)]
    save_receipt: Option<std::path::PathBuf>,
}

#[derive(Debug, Deserialize)]
struct ScenariosFile {
    #[serde(rename = "version")]
    _version: u32,
    #[serde(rename = "agent_id")]
    _agent_id: String,
    scenarios: Vec<Scenario>,
}

#[derive(Debug, Deserialize)]
struct Scenario {
    id: String,
    description: String,
    aprp_fixture: std::path::PathBuf,
    #[serde(default)]
    attack_prompt: Option<String>,
    expected_status: ExpectedStatus,
    #[serde(default)]
    expected_deny_code: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum ExpectedStatus {
    AutoApproved,
    Rejected,
    RequiresHuman,
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let cli = Cli::parse();
    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("research-agent: {e}");
            ExitCode::from(1)
        }
    }
}

fn run(cli: &Cli) -> anyhow::Result<()> {
    if cli.scenario.is_some() && cli.uniswap_quote.is_some() {
        anyhow::bail!("--scenario and --uniswap-quote are mutually exclusive");
    }
    if cli.scenario.is_none() && cli.uniswap_quote.is_none() {
        anyhow::bail!(
            "one of --scenario {{legit-x402,prompt-injection}} or --uniswap-quote <path> is required"
        );
    }

    if let Some(fixture) = &cli.ens_fixture {
        ens_lookup(fixture, &cli.ens_name, cli.policy.as_deref())?;
    }

    let policy = load_policy(cli.policy.as_deref())?;

    if let Some(scenario_id) = &cli.scenario {
        run_scenario(cli, scenario_id, policy)?;
    } else if let Some(quote_path) = &cli.uniswap_quote {
        run_uniswap(cli, quote_path, policy)?;
    }

    Ok(())
}

fn load_policy(override_path: Option<&std::path::Path>) -> anyhow::Result<Policy> {
    if let Some(path) = override_path {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("read policy {}: {e}", path.display()))?;
        let policy = Policy::parse_json(&raw)
            .map_err(|e| anyhow::anyhow!("parse policy {}: {e}", path.display()))?;
        Ok(policy)
    } else {
        Ok(reference_policy())
    }
}

fn run_scenario(cli: &Cli, scenario_id: &str, policy: Policy) -> anyhow::Result<()> {
    let scenarios_dir = cli
        .scenarios
        .parent()
        .ok_or_else(|| anyhow::anyhow!("scenarios.json has no parent"))?
        .to_path_buf();
    let scenarios_raw = std::fs::read_to_string(&cli.scenarios)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", cli.scenarios.display()))?;
    let scenarios: ScenariosFile = serde_json::from_str(&scenarios_raw)?;

    let scenario = scenarios
        .scenarios
        .iter()
        .find(|s| s.id == scenario_id)
        .ok_or_else(|| anyhow::anyhow!("unknown scenario {:?}", scenario_id))?;

    let aprp_path = scenarios_dir.join(&scenario.aprp_fixture);
    let aprp_raw = std::fs::read_to_string(&aprp_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", aprp_path.display()))?;
    let aprp_value: Value = serde_json::from_str(&aprp_raw)?;

    let storage_path = cli.storage_path.clone();
    let runtime = tokio::runtime::Runtime::new()?;
    let response = runtime.block_on(async move {
        call_in_memory(aprp_value, policy, storage_path.as_deref()).await
    })?;

    print_summary(scenario, &response);
    check_expectations(scenario, &response)?;

    if let Some(out) = &cli.save_receipt {
        save_receipt_json(out, &response)?;
    }
    if cli.execute_keeperhub {
        keeperhub_route(&aprp_path, &response)?;
    }
    Ok(())
}

fn run_uniswap(cli: &Cli, quote_path: &std::path::Path, policy: Policy) -> anyhow::Result<()> {
    let quote_raw = std::fs::read_to_string(quote_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", quote_path.display()))?;
    let quote: SwapQuote = serde_json::from_str(&quote_raw)
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", quote_path.display()))?;
    let swap_policy_raw = std::fs::read_to_string(&cli.swap_policy)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", cli.swap_policy.display()))?;
    let swap_policy: SwapPolicy = serde_json::from_str(&swap_policy_raw)
        .map_err(|e| anyhow::anyhow!("parse {}: {e}", cli.swap_policy.display()))?;

    let now_unix = Utc::now().timestamp();
    let outcome = evaluate_swap(&quote, &swap_policy, now_unix, cli.relax_quote_freshness);
    print_swap_outcome(&quote, &outcome);

    let aprp_value = quote_to_aprp(&quote)?;

    let storage_path = cli.storage_path.clone();
    let runtime = tokio::runtime::Runtime::new()?;
    let response = runtime.block_on(async move {
        call_in_memory(aprp_value.clone(), policy, storage_path.as_deref()).await
    })?;
    print_uniswap_summary(&quote, &response);

    if let Some(out) = &cli.save_receipt {
        save_receipt_json(out, &response)?;
    }
    if cli.execute_uniswap {
        uniswap_route(&quote, &response)?;
    }

    let blocked_anywhere =
        outcome.blocked || matches!(response.decision, sbo3l_core::receipt::Decision::Deny);
    let label_is_allow_path =
        matches!(response.decision, sbo3l_core::receipt::Decision::Allow) && !outcome.blocked;
    if !label_is_allow_path && !blocked_anywhere {
        anyhow::bail!(
            "uniswap demo expectation drift: swap_policy_blocked={} sbo3l_decision={:?}",
            outcome.blocked,
            response.decision
        );
    }
    Ok(())
}

fn quote_to_aprp(quote: &SwapQuote) -> anyhow::Result<Value> {
    let amount = quote
        .input
        .amount
        .clone()
        .ok_or_else(|| anyhow::anyhow!("quote.input.amount is required"))?;
    let nonce = ulid::Ulid::new().to_string();
    let expiry = (Utc::now() + Duration::minutes(5)).to_rfc3339();
    let task_id = format!("uniswap-swap-{}", quote.quote_id);
    let body = serde_json::json!({
        "agent_id": "research-agent-01",
        "task_id": task_id,
        "intent": "pay_agent_service",
        "amount": { "value": amount, "currency": "USD" },
        "token": quote.input.token_symbol,
        "destination": {
            "type": "smart_account",
            "address": quote.treasury_recipient,
        },
        "payment_protocol": "smart_account_session",
        "chain": quote.chain,
        "provider_url": "https://api.example.com",
        "x402_payload": null,
        "expiry": expiry,
        "nonce": nonce,
        "expected_result": null,
        "risk_class": "low",
    });
    Ok(body)
}

fn ens_lookup(
    fixture: &std::path::Path,
    name: &str,
    policy_override: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    let resolver = OfflineEnsResolver::from_file(fixture)
        .map_err(|e| anyhow::anyhow!("ENS fixture {}: {e}", fixture.display()))?;
    let records = resolver
        .resolve(name)
        .map_err(|e| anyhow::anyhow!("ENS resolve {name}: {e}"))?;
    println!();
    println!("ens.name:        {name}");
    println!("ens.agent_id:    {}", records.agent_id);
    println!("ens.endpoint:    {}", records.endpoint);
    println!("ens.policy_hash: {}", records.policy_hash);
    println!("ens.audit_root:  {}", records.audit_root);
    let active_policy = load_policy(policy_override)?;
    let active = active_policy
        .canonical_hash()
        .map_err(|e| anyhow::anyhow!("policy hash: {e}"))?;
    records
        .verify_policy_hash(&active)
        .map_err(|e| anyhow::anyhow!("ENS policy_hash mismatch: {e}"))?;
    println!("ens.verify:      ok (matches active policy {})", active);
    Ok(())
}

fn keeperhub_route(
    aprp_path: &std::path::Path,
    response: &PaymentRequestResponse,
) -> anyhow::Result<()> {
    let aprp_raw = std::fs::read_to_string(aprp_path)?;
    let aprp_value: Value = serde_json::from_str(&aprp_raw)?;
    let aprp: sbo3l_core::aprp::PaymentRequest = serde_json::from_value(aprp_value)?;
    let executor = KeeperHubExecutor::local_mock();
    println!();
    match executor.execute(&aprp, &response.receipt) {
        Ok(receipt) => {
            println!("keeperhub.sponsor:       {}", receipt.sponsor);
            println!("keeperhub.execution_ref: {}", receipt.execution_ref);
            println!("keeperhub.mock:          {}", receipt.mock);
            println!("keeperhub.note:          {}", receipt.note);
        }
        Err(e) => {
            println!("keeperhub.sponsor:       keeperhub");
            println!("keeperhub.refused:       {e}");
            println!("keeperhub.note:          denied actions never reach the sponsor");
        }
    }
    Ok(())
}

fn uniswap_route(quote: &SwapQuote, response: &PaymentRequestResponse) -> anyhow::Result<()> {
    let aprp_value = quote_to_aprp(quote)?;
    let aprp: sbo3l_core::aprp::PaymentRequest = serde_json::from_value(aprp_value)?;
    let executor = UniswapExecutor::local_mock();
    println!();
    match executor.execute(&aprp, &response.receipt) {
        Ok(receipt) => {
            println!("uniswap.sponsor:       {}", receipt.sponsor);
            println!("uniswap.execution_ref: {}", receipt.execution_ref);
            println!("uniswap.mock:          {}", receipt.mock);
            println!("uniswap.note:          {}", receipt.note);
        }
        Err(e) => {
            println!("uniswap.sponsor:       uniswap");
            println!("uniswap.refused:       {e}");
            println!("uniswap.note:          denied actions never reach the sponsor");
        }
    }
    Ok(())
}

fn save_receipt_json(
    out: &std::path::Path,
    response: &PaymentRequestResponse,
) -> anyhow::Result<()> {
    let body = serde_json::to_vec_pretty(&response.receipt)?;
    std::fs::write(out, body)
        .map_err(|e| anyhow::anyhow!("write receipt to {}: {e}", out.display()))?;
    Ok(())
}

async fn call_in_memory(
    aprp: Value,
    policy: Policy,
    storage_path: Option<&std::path::Path>,
) -> anyhow::Result<PaymentRequestResponse> {
    let storage = match storage_path {
        Some(p) => {
            Storage::open(p).map_err(|e| anyhow::anyhow!("open storage at {}: {e}", p.display()))?
        }
        None => Storage::open_in_memory()?,
    };
    let state = AppState::new(policy, storage);
    let app = sbo3l_server::router(state);

    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    let req = Request::builder()
        .method("POST")
        .uri("/v1/payment-requests")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&aprp)?))?;
    let resp = app.oneshot(req).await?;
    let status = resp.status();
    let body_bytes = resp.into_body().collect().await?.to_bytes();
    if !status.is_success() {
        let v: Value = serde_json::from_slice(&body_bytes).unwrap_or(Value::Null);
        anyhow::bail!("HTTP {status}: {v}");
    }
    let parsed: PaymentRequestResponse = serde_json::from_slice(&body_bytes)?;
    Ok(parsed)
}

fn print_summary(scenario: &Scenario, response: &PaymentRequestResponse) {
    println!("scenario:      {}", scenario.id);
    println!("description:   {}", scenario.description);
    if let Some(p) = &scenario.attack_prompt {
        println!("attack_prompt: {p}");
    }
    println!("status:        {:?}", response.status);
    println!("decision:      {:?}", response.decision);
    if let Some(c) = &response.deny_code {
        println!("deny_code:     {c}");
    }
    if let Some(r) = &response.matched_rule_id {
        println!("matched_rule:  {r}");
    }
    println!("request_hash:  {}", response.request_hash);
    println!("policy_hash:   {}", response.policy_hash);
    println!("audit_event:   {}", response.audit_event_id);
    println!(
        "receipt_sig:   {}",
        response.receipt.signature.signature_hex
    );
}

fn print_swap_outcome(quote: &SwapQuote, outcome: &SwapPolicyOutcome) {
    println!();
    println!("uniswap.quote_id:        {}", quote.quote_id);
    println!(
        "uniswap.swap:            {} {} -> {} {}",
        quote.input.amount.clone().unwrap_or_default(),
        quote.input.token_symbol,
        quote.output.amount.clone().unwrap_or_default(),
        quote.output.token_symbol,
    );
    println!("uniswap.recipient:       {}", quote.treasury_recipient);
    println!("uniswap.slippage_bps:    {}", quote.expected_slippage_bps);
    println!("uniswap.swap_policy_blocked: {}", outcome.blocked);
    for c in &outcome.checks {
        let tag = if c.ok { "ok  " } else { "FAIL" };
        let relax = if c.relaxed { " (relaxed)" } else { "" };
        println!("  {tag} {}{} — {}", c.name, relax, c.detail);
    }
}

fn print_uniswap_summary(quote: &SwapQuote, response: &PaymentRequestResponse) {
    println!();
    println!("uniswap.aprp.task_id:    uniswap-swap-{}", quote.quote_id);
    println!("uniswap.sbo3l.status:  {:?}", response.status);
    println!("uniswap.sbo3l.decision:{:?}", response.decision);
    if let Some(c) = &response.deny_code {
        println!("uniswap.sbo3l.deny_code:    {c}");
    }
    if let Some(r) = &response.matched_rule_id {
        println!("uniswap.sbo3l.matched_rule: {r}");
    }
    println!("uniswap.sbo3l.request_hash: {}", response.request_hash);
    println!("uniswap.sbo3l.policy_hash:  {}", response.policy_hash);
    println!("uniswap.sbo3l.audit_event:  {}", response.audit_event_id);
}

fn check_expectations(
    scenario: &Scenario,
    response: &PaymentRequestResponse,
) -> anyhow::Result<()> {
    use sbo3l_server::PaymentStatus;
    let expected_status = match scenario.expected_status {
        ExpectedStatus::AutoApproved => PaymentStatus::AutoApproved,
        ExpectedStatus::Rejected => PaymentStatus::Rejected,
        ExpectedStatus::RequiresHuman => PaymentStatus::RequiresHuman,
    };
    if response.status != expected_status {
        anyhow::bail!(
            "expected status {:?}, got {:?}",
            expected_status,
            response.status
        );
    }
    if let Some(expected_code) = &scenario.expected_deny_code {
        // The reference fixture lists exactly one expected deny_code per scenario,
        // but the README permits either deny_unknown_provider or
        // deny_recipient_not_allowlisted for the prompt-injection scenario.
        let acceptable: Vec<&str> = if scenario.id == "prompt-injection" {
            vec![
                "policy.deny_unknown_provider",
                "policy.deny_recipient_not_allowlisted",
            ]
        } else {
            vec![expected_code.as_str()]
        };
        match &response.deny_code {
            Some(actual) if acceptable.iter().any(|c| c == actual) => Ok(()),
            Some(actual) => Err(anyhow::anyhow!(
                "deny_code {actual} not in acceptable {:?}",
                acceptable
            )),
            None => Err(anyhow::anyhow!(
                "expected deny_code in {:?}, got none",
                acceptable
            )),
        }
    } else {
        if response.deny_code.is_some() {
            anyhow::bail!(
                "expected no deny_code, got {:?}",
                response.deny_code.as_ref()
            );
        }
        Ok(())
    }
}
