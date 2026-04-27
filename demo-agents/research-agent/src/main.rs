//! ETHGlobal Open Agents demo harness.
//!
//! Drives Mandate's payment-request pipeline end-to-end against the
//! in-memory daemon for the two locked scenarios: `legit-x402` and
//! `prompt-injection`. Output is deterministic — no LLM credentials, no
//! network calls.

use std::process::ExitCode;

use clap::Parser;
use serde::Deserialize;
use serde_json::Value;

use mandate_execution::{GuardedExecutor, KeeperHubExecutor};
use mandate_identity::{EnsResolver, OfflineEnsResolver};
use mandate_server::{reference_policy, AppState, PaymentRequestResponse};
use mandate_storage::Storage;

#[derive(Parser, Debug)]
#[command(
    name = "research-agent",
    version,
    about = "Mandate ETHGlobal Open Agents research-agent harness."
)]
struct Cli {
    /// Scenario id from scenarios.json: legit-x402 | prompt-injection
    #[arg(long)]
    scenario: String,
    /// Path to scenarios.json (defaults to next to the binary)
    #[arg(long, default_value = "demo-agents/research-agent/scenarios.json")]
    scenarios: std::path::PathBuf,

    /// Optional ENS records fixture (`{"name.eth": {...}}`). When set, resolve
    /// `--ens-name` and verify the `mandate:policy_hash` text record matches
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
        .find(|s| s.id == cli.scenario)
        .ok_or_else(|| anyhow::anyhow!("unknown scenario {:?}", cli.scenario))?;

    let aprp_path = scenarios_dir.join(&scenario.aprp_fixture);
    let aprp_raw = std::fs::read_to_string(&aprp_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", aprp_path.display()))?;
    let aprp_value: Value = serde_json::from_str(&aprp_raw)?;

    if let Some(fixture) = &cli.ens_fixture {
        ens_lookup(fixture, &cli.ens_name)?;
    }

    let runtime = tokio::runtime::Runtime::new()?;
    let response = runtime.block_on(async move { call_in_memory(aprp_value).await })?;

    print_summary(scenario, &response);
    check_expectations(scenario, &response)?;

    if cli.execute_keeperhub {
        keeperhub_route(&aprp_path, &response)?;
    }
    Ok(())
}

fn ens_lookup(fixture: &std::path::Path, name: &str) -> anyhow::Result<()> {
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
    let active = reference_policy()
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
    let aprp: mandate_core::aprp::PaymentRequest = serde_json::from_value(aprp_value)?;
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

async fn call_in_memory(aprp: Value) -> anyhow::Result<PaymentRequestResponse> {
    let storage = Storage::open_in_memory()?;
    let policy = reference_policy();
    let state = AppState::new(policy, storage);
    let app = mandate_server::router(state);

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

fn check_expectations(
    scenario: &Scenario,
    response: &PaymentRequestResponse,
) -> anyhow::Result<()> {
    use mandate_server::PaymentStatus;
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
