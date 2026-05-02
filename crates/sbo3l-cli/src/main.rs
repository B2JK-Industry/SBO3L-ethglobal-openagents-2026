use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use sbo3l_core::audit::{verify_chain, SignedAuditEvent};
use sbo3l_core::audit_bundle::{self, AuditBundle};
use sbo3l_core::receipt::PolicyReceipt;
use sbo3l_core::{schema, SchemaError};

mod admin_backup;
mod agent;
#[cfg(feature = "eth_broadcast")]
mod agent_broadcast;
mod agent_reputation;
mod agent_reputation_aggregate;
#[cfg(feature = "eth_broadcast")]
mod agent_reputation_broadcast;
#[cfg(feature = "eth_broadcast")]
mod agent_reputation_multichain;
mod agent_verify;
mod audit_anchor;
mod audit_anchor_ens;
mod audit_checkpoint;
mod audit_verify_anchor;
mod doctor;
mod doctor_extended;
mod key;
mod passport;
mod policy;

#[derive(Parser, Debug)]
#[command(
    name = "sbo3l",
    version,
    about = "SBO3L — spending mandates for autonomous agents.",
    long_about = "SBO3L is a local policy, budget, receipt and audit firewall for AI agents.\n\
                  Public brand: SBO3L. Tagline: Don't give your agent a wallet. Give it a mandate."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Agent Payment Request Protocol commands
    Aprp {
        #[command(subcommand)]
        op: AprpCmd,
    },
    /// Verify a SBO3L audit hash chain (JSONL)
    VerifyAudit {
        /// Path to a JSONL audit log
        #[arg(long)]
        path: PathBuf,
        /// Skip recomputation of event_hash (for fixtures with placeholder hashes)
        #[arg(long, default_value_t = false)]
        skip_hash: bool,
        /// Public key (hex, 32 bytes) to verify each event's signature
        #[arg(long)]
        pubkey: Option<String>,
    },
    /// Print the schema id for a wire format
    Schema {
        /// One of: aprp | policy | decision-token | policy-receipt | audit-event | x402
        kind: String,
    },
    /// Verifiable audit export bundle commands.
    ///
    /// `sbo3l audit export` packages a signed receipt + the relevant audit
    /// chain segment + the public verification keys into a single JSON file
    /// that anyone can re-verify offline. `sbo3l audit verify-bundle`
    /// re-derives every signature, hash and chain link in that file and
    /// reports the result. Tagline: SBO3L does not just decide. It leaves
    /// behind verifiable proof.
    Audit {
        #[command(subcommand)]
        op: AuditCmd,
    },
    /// Operator readiness summary.
    ///
    /// Inspects a SBO3L SQLite database (or an in-memory fresh one) and
    /// reports per-feature status: storage open, migrations applied, audit
    /// chain integrity, nonce-replay table, idempotency table, mock KMS
    /// keyring, active policy. Each check is **honest about scope** — a
    /// feature that is not implemented yet surfaces as `skip`, never as
    /// fake `ok`. Output is a human-readable summary by default; `--json`
    /// emits a machine-readable envelope suitable for pipelines and the
    /// production-shaped runner.
    Doctor {
        /// Path to a SBO3L SQLite database. If omitted, opens a fresh
        /// in-memory database (every check runs against a clean slate —
        /// useful for verifying the binary itself works).
        #[arg(long)]
        db: Option<PathBuf>,
        /// Emit JSON instead of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
        /// Extended mode: also probe the 6 SBO3L Sepolia contracts
        /// (`OffchainResolver`, `AnchorRegistry`, `SubnameAuction`,
        /// `ReputationBond`, `ReputationRegistry`,
        /// `ERC8004 IdentityRegistry`) — runs `eth_getCode` plus one
        /// view call per contract. The OffchainResolver probe also
        /// validates the URL template carries `{sender}` + `{data}`
        /// (the shape Heidi's Bug #2 broke at submission time).
        ///
        /// RPC URL resolution: `--rpc-url` flag, then
        /// `SBO3L_SEPOLIA_RPC_URL` env, then `SBO3L_RPC_URL` env, then
        /// PublicNode public endpoint as last-resort. Alchemy
        /// preferred per `memory:alchemy_rpc_endpoints.md`.
        #[arg(long, default_value_t = false)]
        extended: bool,
        /// Sepolia JSON-RPC URL for `--extended` probes. When omitted,
        /// the resolver falls back through `SBO3L_SEPOLIA_RPC_URL` →
        /// `SBO3L_RPC_URL` → PublicNode. Ignored without `--extended`.
        #[arg(long)]
        rpc_url: Option<String>,
    },
    /// Mock KMS keyring commands (PSM-A1.9).
    ///
    /// Operate on the persistent `mock_kms_keys` SQLite table (V005).
    /// Every operation requires `--mock` for explicit disclosure — these
    /// commands are NOT plug-compatible with a production KMS. See
    /// `docs/cli/mock-kms.md`.
    Key {
        #[command(subcommand)]
        op: KeyCmd,
    },
    /// Local active-policy lifecycle (PSM-A3).
    ///
    /// Operates on the persistent `active_policy` SQLite table (V006).
    /// This is **local production-shaped lifecycle**, not remote
    /// governance: there is no on-chain anchor, no consensus, no
    /// signing on activation; whoever opens the DB activates the
    /// policy. See `docs/cli/policy.md`.
    Policy {
        #[command(subcommand)]
        op: PolicyCmd,
    },
    /// SBO3L Passport — portable proof capsule.
    ///
    /// `sbo3l passport verify --path <capsule>` runs schema and
    /// cross-field structural verification against a
    /// `sbo3l.passport_capsule.{v1,v2}` artifact, and auto-promotes
    /// to crypto verification when the capsule is self-contained.
    /// `passport run` orchestrates the offline flow end-to-end and
    /// emits a capsule. `passport explain` prints a human-readable
    /// summary. Source of truth:
    /// `docs/product/SBO3L_PASSPORT_SOURCE_OF_TRUTH.md`.
    Passport {
        #[command(subcommand)]
        op: PassportCmd,
    },
    /// Agent ENS lifecycle (T-3-1).
    ///
    /// Currently ships `register` for issuing a Durin subname under
    /// a parent (default `sbo3lagent.eth` mainnet) plus a
    /// `multicall(setText × N)` to set every `sbo3l:*` text record
    /// in one tx. T-3-1 main PR ships the `--dry-run` path; broadcast
    /// is gated and lands in a follow-up that wires
    /// `sbo3l_core::signers::eth::EthSigner`.
    ///
    /// See `docs/cli/agent.md`.
    Agent {
        #[command(subcommand)]
        op: AgentCmd,
    },
    /// Operator-level admin commands: backup, restore, export, verify
    /// (R14 P2). Compression + encryption are gated behind
    /// `--features admin_backup` to keep the default CLI binary small.
    /// See `docs/cli/admin-backup.md`.
    Admin {
        #[command(subcommand)]
        op: AdminCmd,
    },
}

#[derive(Subcommand, Debug)]
enum AdminCmd {
    /// Snapshot the SBO3L SQLite DB to a tar.zst archive (optionally
    /// age-encrypted). Uses SQLite VACUUM INTO for a consistent
    /// snapshot without holding a write lock — safe against a live
    /// daemon.
    Backup {
        /// Source SQLite DB path.
        #[arg(long)]
        db: PathBuf,
        /// Destination archive (local path or `file://` URI). `s3://`
        /// is parsed but not yet implemented; use a local path.
        #[arg(long)]
        to: String,
        /// Optional: age recipient string (e.g. `age1...`) or path to
        /// a recipients file. Wraps the archive in an age envelope.
        #[arg(long)]
        encrypt_with: Option<String>,
    },
    /// Restore an archive into a fresh SQLite DB path. Refuses to
    /// overwrite an existing file at `--db`.
    Restore {
        #[arg(long)]
        from: String,
        #[arg(long)]
        db: PathBuf,
        /// Path to an age identity file. Required when the archive
        /// is age-encrypted.
        #[arg(long)]
        decrypt_with: Option<PathBuf>,
    },
    /// Export the audit chain. `--format json` emits one JSON object
    /// per line (JSONL) to `--to`, suitable for downstream pipelines.
    /// `--format parquet` is reserved but errors today (arrow-rs adds
    /// a 50+ crate dep tree; deferred).
    Export {
        #[arg(long)]
        db: PathBuf,
        #[arg(long)]
        to: String,
        /// `json` | `parquet` (`parquet` errors today). Use `-` as
        /// `--to` to write to stdout.
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Verify an archive: walk the audit chain and assert every
    /// `prev_event_hash` link is intact. Read-only; doesn't restore.
    Verify {
        #[arg(long)]
        from: String,
        #[arg(long)]
        decrypt_with: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum AgentCmd {
    /// Issue an ENS subname `<name>.<parent>` and pre-pack a
    /// `multicall(setText × N)` to set every `sbo3l:*` record.
    ///
    /// Default `--parent sbo3lagent.eth` (mainnet). Default
    /// `--network sepolia`; mainnet requires `SBO3L_ALLOW_MAINNET_TX=1`
    /// and an explicit `--network mainnet`.
    Register {
        /// Single DNS label (no `.`). E.g. `research-agent`.
        #[arg(long)]
        name: String,

        /// Parent ENS name. Default `sbo3lagent.eth`.
        #[arg(long, default_value = agent::DEFAULT_PARENT)]
        parent: String,

        /// `mainnet` or `sepolia`. Default `sepolia`.
        #[arg(long, default_value = "sepolia")]
        network: String,

        /// JSON object mapping `sbo3l:<key>` → value. Non-`sbo3l:*`
        /// keys are refused.
        #[arg(long)]
        records: String,

        /// On-chain owner of the subname after issuance. EIP-55 hex
        /// with `0x` prefix. Required in this build; defaults to the
        /// signer address once the EthSigner factory wires up.
        #[arg(long)]
        owner: Option<String>,

        /// Override the resolver address. Default = the network's
        /// canonical PublicResolver.
        #[arg(long)]
        resolver: Option<String>,

        /// **Not implemented in this build.** Stub returns a clear
        /// error. Future broadcast path requires `--rpc-url` and
        /// `--private-key-env-var`.
        #[arg(long, default_value_t = false)]
        broadcast: bool,

        /// Explicitly request dry-run (no broadcast). Dry-run is
        /// already the default, but passing `--dry-run` surfaces
        /// intent — automation scripts pass it as defense-in-depth so
        /// a future flip of the CLI default to broadcast won't
        /// silently turn an envelope-build invocation into a real tx.
        /// Mutually exclusive with `--broadcast`.
        #[arg(long, default_value_t = false, conflicts_with = "broadcast")]
        dry_run: bool,

        #[arg(long)]
        rpc_url: Option<String>,

        #[arg(long)]
        private_key_env_var: Option<String>,

        /// Write the dry-run envelope to `<path>` as JSON in addition
        /// to printing.
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Verify the ENS records of an SBO3L agent (pair to `register`).
    ///
    /// Resolves all canonical `sbo3l:*` text records for the supplied
    /// FQDN via `LiveEnsResolver` and asserts each present record
    /// matches the operator's expectations.
    ///
    /// Pass `--expected-pubkey 0x<64-hex>` (or derive from
    /// `--key-file <path>`) to assert the agent's
    /// `sbo3l:pubkey_ed25519` record matches a known identity.
    /// Pass `--expected-records '<json>'` for per-record assertions
    /// against any `sbo3l:*` key.
    ///
    /// Exit codes: 0 PASS / 2 FAIL / 1 resolution error.
    /// See `docs/cli/agent-verify.md`.
    VerifyEns {
        /// Fully-qualified ENS name (e.g.
        /// `research-agent.sbo3lagent.eth`).
        fqdn: String,

        /// `mainnet` or `sepolia`. Default `mainnet` (the live name).
        #[arg(long, default_value = "mainnet")]
        network: String,

        /// Override the resolver RPC. Default = `SBO3L_ENS_RPC_URL`.
        #[arg(long)]
        rpc_url: Option<String>,

        /// `0x` + 64 hex chars Ed25519 pubkey. Asserts against
        /// `sbo3l:pubkey_ed25519`. Mutually exclusive with
        /// `--key-file`.
        #[arg(long)]
        expected_pubkey: Option<String>,

        /// Path to a local Ed25519 secret seed (32 raw bytes OR 64
        /// hex chars in UTF-8). Pubkey is derived and asserted.
        #[arg(long, conflicts_with = "expected_pubkey")]
        key_file: Option<PathBuf>,

        /// JSON object `{"sbo3l:agent_id":"...", ...}` of expected
        /// records. Records not listed are reported but not failed.
        ///
        /// Default: **strict mode** — any key outside the canonical
        /// `sbo3l:*` set causes verify-ens to refuse with exit code
        /// 2, so a typo (`sbol3:agent_id`) doesn't silently turn
        /// into a no-op expectation. Pass `--lenient` to opt back
        /// into the legacy silent-ignore behaviour.
        #[arg(long)]
        expected_records: Option<String>,

        /// Silently ignore unknown keys in `--expected-records`.
        /// Disables the strict-mode default. Useful when an upstream
        /// pipeline injects extra metadata that's not part of
        /// SBO3L's canonical record set.
        #[arg(long, default_value_t = false)]
        lenient: bool,

        /// Emit a `sbo3l.verify_ens_report.v1` JSON envelope instead
        /// of human-readable text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
    /// Compute and publish an agent's reputation score (T-4-6 / T-4-7).
    ///
    /// Reads audit events from `--events <file.json>` (a JSON array
    /// of {decision, executor_confirmed, age_secs} objects), computes
    /// the v2 weighted score (sbo3l_policy::reputation::compute_reputation_v2),
    /// and emits a setText envelope publishing the score to the agent's
    /// `sbo3l:reputation_score` ENS text record.
    ///
    /// **Dry-run by default.** Pass `--broadcast` (requires
    /// `--features eth_broadcast` at build time) to actually sign +
    /// send the setText tx via the alloy harness shared with T-3-1
    /// agent-register broadcast. Mainnet path additionally requires
    /// `SBO3L_ALLOW_MAINNET_TX=1` and an explicit `--network mainnet`.
    ReputationPublish {
        /// FQDN of the agent (e.g. `research-agent.sbo3lagent.eth`).
        #[arg(long)]
        fqdn: String,

        /// Path to a JSON file: array of `ReputationEventInput`
        /// (`{decision, executor_confirmed, age_secs}`).
        #[arg(long)]
        events: PathBuf,

        /// `mainnet` or `sepolia`. Default `mainnet` since the
        /// canonical reputation publishes are on `sbo3lagent.eth`.
        #[arg(long, default_value = "mainnet")]
        network: String,

        /// Override the resolver address. Default = the network's
        /// canonical PublicResolver.
        #[arg(long)]
        resolver: Option<String>,

        /// Write the envelope JSON to `<path>` in addition to printing.
        #[arg(long)]
        out: Option<PathBuf>,

        /// **Sign + send the setText tx** instead of just printing
        /// the envelope. Requires the `eth_broadcast` Cargo feature;
        /// without it the dispatch falls through to a clear "rebuild
        /// with --features eth_broadcast" error (exit code 3).
        #[arg(long, default_value_t = false)]
        broadcast: bool,

        /// JSON-RPC URL for `--broadcast`. Falls back to
        /// `SBO3L_RPC_URL` env. Validated http/https.
        #[arg(long)]
        rpc_url: Option<String>,

        /// Override the env var that holds the 32-byte hex private
        /// key for `--broadcast` (default `SBO3L_SIGNER_KEY`).
        #[arg(long)]
        private_key_env_var: Option<String>,

        /// **Multi-chain broadcast (R11 P2).** Comma-separated list
        /// of chain labels — e.g. `--multi-chain sepolia,optimism-sepolia,base-sepolia`.
        /// When set, the same score is broadcast to every chain in
        /// the list. Per-chain RPC URLs come from
        /// `SBO3L_RPC_URL_<UPPERCASE_LABEL>` env vars (e.g.
        /// `SBO3L_RPC_URL_SEPOLIA`). Mainnet entries require
        /// `SBO3L_ALLOW_MAINNET_TX=1`. Implies `--broadcast`.
        #[arg(long)]
        multi_chain: Option<String>,
    },

    /// Aggregate cross-chain reputation snapshots into one score (R12 P3).
    ///
    /// Reads a JSON file describing per-chain snapshots
    /// (`{chain_id, fqdn, score, observed_at}` plus a `now_secs`
    /// timestamp) and prints the
    /// `sbo3l.reputation_aggregate_report.v1` envelope. Pure-offline
    /// reader: the operator gathers the per-chain scores ahead of
    /// time (typically via N parallel `cast call`
    /// `SBO3LReputationRegistry.reputationOf` reads) and feeds them
    /// in.
    ///
    /// Aggregation logic lives in
    /// `sbo3l_policy::cross_chain_reputation::aggregate_reputation`
    /// (R10 #222). Default chain weights: mainnet 1.0, L2s 0.8, etc.
    /// — see the policy crate doc for the full table.
    ReputationAggregate {
        /// Path to a JSON file with `now_secs` + `snapshots: [...]`.
        #[arg(long)]
        input: PathBuf,

        /// Write the aggregate-report JSON to `<path>` in addition
        /// to printing.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum PassportCmd {
    /// Verify a `sbo3l.passport_capsule.v1` JSON artifact against
    /// the embedded schema and the cross-field truthfulness rules
    /// (deny→no execution, live→evidence, request/policy hash
    /// internal-consistency, etc.).
    ///
    /// Default mode is **structural-only** for backwards compat —
    /// schema + cross-field invariants only, no cryptographic
    /// re-verification. Pass `--strict` (alias `--verify-cryptographically`)
    /// to additionally recompute `request_hash` from the capsule's
    /// embedded APRP, recompute `policy_hash` from a supplied policy
    /// snapshot (`--policy`), verify the receipt's Ed25519 signature
    /// against a supplied pubkey (`--receipt-pubkey`), and walk the
    /// audit chain in a supplied bundle (`--audit-bundle`). Each crypto
    /// check whose auxiliary input is absent is reported as
    /// `Skipped(reason)` rather than failed — never a fake-OK.
    Verify {
        /// Path to a capsule JSON file.
        #[arg(long)]
        path: PathBuf,

        /// Run the cryptographic strict verifier on top of the
        /// structural pass. Each crypto check that requires an
        /// absent auxiliary input is reported as `Skipped(reason)`.
        #[arg(long, alias = "verify-cryptographically")]
        strict: bool,

        /// Hex-encoded Ed25519 public key for the receipt signer.
        /// Required for the `receipt_signature` strict check;
        /// otherwise that check is skipped.
        #[arg(long, requires = "strict")]
        receipt_pubkey: Option<String>,

        /// Path to a `sbo3l.audit_bundle.v1` JSON file whose chain
        /// segment contains the capsule's `audit.audit_event_id`.
        /// Required for the `audit_chain` and `audit_event_link`
        /// strict checks; otherwise both are skipped.
        #[arg(long, requires = "strict")]
        audit_bundle: Option<PathBuf>,

        /// Path to the canonical policy JSON snapshot whose JCS+SHA-256
        /// hash should match `capsule.policy.policy_hash`. Required for
        /// the `policy_hash_recompute` strict check; otherwise skipped.
        #[arg(long, requires = "strict")]
        policy: Option<PathBuf>,
    },
    /// Run an APRP through the existing SBO3L offline pipeline
    /// (schema → request_hash → policy → budget → audit → signed
    /// receipt) and emit one `sbo3l.passport_capsule.v1` JSON to
    /// `--out`. Wraps existing primitives — no policy/audit/crypto
    /// rewrite. Supports `--mode mock` only; `--mode live` is
    /// rejected (live executor integration runs in the daemon, not
    /// in this offline CLI surface).
    Run {
        /// Path to an APRP JSON file (the request body the agent
        /// would normally POST to `/v1/payment-requests`).
        aprp: PathBuf,
        /// SBO3L SQLite database path. The active policy is
        /// looked up here via the PSM-A3 storage API.
        #[arg(long)]
        db: PathBuf,
        /// ENS-style agent name (e.g. `research-agent.team.eth`).
        /// Looked up in the ENS fixture; the resulting records map
        /// is captured into the capsule's `agent.records` block.
        #[arg(long)]
        agent: String,
        /// How `agent.records` are obtained. The offline `passport run`
        /// surface uses `offline-fixture`; live ENS resolution flows
        /// through `sbo3l agent verify-ens` instead.
        #[arg(long, value_enum, default_value_t = ResolverChoiceArg::OfflineFixture)]
        resolver: ResolverChoiceArg,
        /// Path to the ENS fixture. Required when
        /// `--resolver offline-fixture`.
        #[arg(long)]
        ens_fixture: Option<PathBuf>,
        /// Mock executor that receives the allow-path receipt.
        /// Deny-path capsules never call the executor regardless of
        /// this value (status=not_called is hard-enforced).
        #[arg(long, value_enum)]
        executor: ExecutorChoiceArg,
        /// Execution mode. This offline CLI only supports `mock`.
        /// `live` is rejected with exit 2 (truthfulness rule: live
        /// claims require real evidence). For real live execution,
        /// use the daemon (`sbo3l-server`) with configured executor
        /// credentials.
        #[arg(long, value_enum, default_value_t = ModeChoiceArg::Mock)]
        mode: ModeChoiceArg,
        /// Output path for the capsule JSON. Written atomically
        /// (tempfile + rename); never leaves a half-written file.
        #[arg(long)]
        out: PathBuf,
        /// F-6: capsule schema version. `v2` (default) embeds
        /// `policy.policy_snapshot` + `audit.audit_segment` so a
        /// downstream `passport verify --strict` runs all 6
        /// cryptographic checks WITHOUT auxiliary inputs. `v1` emits
        /// the legacy shape (no embedded fields; strict mode requires
        /// `--policy`, `--audit-bundle`, `--receipt-pubkey` to cover
        /// the same ground).
        #[arg(long = "schema-version", value_enum, default_value_t = SchemaVersionArg::V2)]
        schema_version: SchemaVersionArg,
    },
    /// Verify a capsule and print a concise human (or `--json`)
    /// summary suitable for judges and operators.
    Explain {
        /// Path to a capsule JSON file.
        #[arg(long)]
        path: PathBuf,
        /// Emit JSON instead of human text.
        #[arg(long, default_value_t = false)]
        json: bool,
    },
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum ResolverChoiceArg {
    OfflineFixture,
    LiveEns,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum ExecutorChoiceArg {
    Keeperhub,
    Uniswap,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum ModeChoiceArg {
    Mock,
    Live,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy)]
enum SchemaVersionArg {
    V1,
    V2,
}

#[derive(Subcommand, Debug)]
enum PolicyCmd {
    /// Parse + semantic-validate + canonical-hash a policy JSON file.
    /// Stdout: policy_hash + summary counts. No DB access.
    Validate {
        /// Path to a policy JSON file.
        path: PathBuf,
    },
    /// Print the currently-active policy row from the DB. Exits non-
    /// zero (code 3) if no policy has been activated yet — that is the
    /// honest signal, not a fake "ok".
    Current {
        /// SQLite database path.
        #[arg(long)]
        db: PathBuf,
    },
    /// Validate, hash, and activate a policy. Idempotent: re-running
    /// with the same policy is a no-op.
    Activate {
        /// Path to a policy JSON file.
        path: PathBuf,
        /// SQLite database path.
        #[arg(long)]
        db: PathBuf,
        /// Optional source label recorded in the row (default
        /// `operator-cli`).
        #[arg(long)]
        source: Option<String>,
    },
    /// Diff two candidate policy files at the canonical-JSON level.
    /// Exits 0 if identical, 1 if they differ (with a printed diff),
    /// 2 if either file fails to parse / validate.
    Diff {
        /// Left-hand policy file ("from").
        a: PathBuf,
        /// Right-hand policy file ("to").
        b: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum KeyCmd {
    /// Initialise a mock keyring's v1 row for the given `--role`.
    /// Idempotent: running again with the same args is a no-op.
    Init {
        /// Required acknowledgement that this is mock KMS infrastructure.
        #[arg(long)]
        mock: bool,
        /// Stable role name (e.g. `audit-mock`, `decision-mock`).
        #[arg(long)]
        role: String,
        /// 32-byte deterministic root seed, hex-encoded (64 chars). The
        /// seed never enters the SQLite database — only its derived
        /// public keys do.
        #[arg(long)]
        root_seed: String,
        /// Optional v1 timestamp (RFC3339). Defaults to "now()".
        #[arg(long)]
        genesis: Option<String>,
        /// SQLite database path (the same one the daemon writes to).
        #[arg(long)]
        db: PathBuf,
    },
    /// List keyring rows in `(role, version)` order.
    List {
        #[arg(long)]
        mock: bool,
        /// Restrict to a single role.
        #[arg(long)]
        role: Option<String>,
        #[arg(long)]
        db: PathBuf,
    },
    /// Add the next version of `--role` to the keyring. Reads the
    /// existing maximum version, derives the new version's public
    /// material from `(role, n+1, root_seed)`, inserts the row.
    Rotate {
        #[arg(long)]
        mock: bool,
        #[arg(long)]
        role: String,
        #[arg(long)]
        root_seed: String,
        #[arg(long)]
        db: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum AuditCmd {
    /// Build a verifiable bundle from a signed receipt + audit chain.
    ///
    /// Exactly one chain source must be supplied:
    ///   --chain <jsonl-path>  reads SignedAuditEvent[] from a JSONL file
    ///                         (one event per line, genesis through the
    ///                         receipt's `audit_event_id`, in seq order).
    ///   --db    <sqlite-path> reads the chain directly from a SBO3L
    ///                         daemon's SQLite storage (`sbo3l-storage`),
    ///                         slicing the prefix through the receipt's
    ///                         `audit_event_id`. Performs a pre-flight
    ///                         `verify_chain` and a receipt-signature
    ///                         check before writing the bundle.
    Export {
        /// Path to the signed PolicyReceipt JSON (the body returned by
        /// `POST /v1/payment-requests`, field `receipt`).
        #[arg(long)]
        receipt: PathBuf,
        /// Path to a JSONL audit chain (one SignedAuditEvent per line).
        /// Mutually exclusive with `--db`; exactly one must be supplied.
        #[arg(long, conflicts_with = "db", required_unless_present = "db")]
        chain: Option<PathBuf>,
        /// Path to a SBO3L SQLite storage file (the `SBO3L_DB` the
        /// daemon writes to). Mutually exclusive with `--chain`; exactly
        /// one must be supplied. Reads the audit chain prefix through
        /// the receipt's `audit_event_id` directly from the daemon's
        /// persisted log — no out-of-band JSONL export required.
        #[arg(long, conflicts_with = "chain", required_unless_present = "chain")]
        db: Option<PathBuf>,
        /// Public verification key (hex) for the receipt signer (32 bytes).
        #[arg(long)]
        receipt_pubkey: String,
        /// Public verification key (hex) for the audit signer (32 bytes).
        #[arg(long)]
        audit_pubkey: String,
        /// Output path. If omitted, the bundle JSON is written to stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Where to publish the bundle. `local` (default) writes the bundle
        /// JSON to disk via `--out` (or stdout when omitted), preserving the
        /// pre-existing CLI behaviour exactly. `0g-storage` additionally
        /// uploads the bundle to the 0G Galileo testnet indexer (see
        /// `SBO3L_ZEROG_INDEXER_URL`) and prints the returned `rootHash`.
        ///
        /// 0G testnet is documented-flaky; a successful upload is
        /// best-effort, not a guarantee. On hard failure the CLI points
        /// the operator at the browser-upload tool at
        /// `https://storagescan-galileo.0g.ai/tool` as a fallback.
        #[arg(long, value_parser = ["local", "0g-storage"], default_value = "local")]
        backend: String,
        /// 0G Storage indexer URL (only consulted when `--backend 0g-storage`).
        /// Defaults to the env var `SBO3L_ZEROG_INDEXER_URL`, then falls back
        /// to the Galileo testnet turbo indexer baked into the build.
        #[arg(long)]
        zerog_indexer_url: Option<String>,
    },
    /// Verify a previously-exported bundle.
    ///
    /// Re-derives every receipt + audit signature, every audit event_hash,
    /// and the prev_event_hash linkage of the included chain segment. Exits
    /// with code 0 on success, 1 on any verification failure, 2 on I/O or
    /// JSON-parse errors.
    VerifyBundle {
        /// Path to a bundle JSON file produced by `sbo3l audit export`.
        #[arg(long)]
        path: PathBuf,
    },
    /// **Mock-anchored** audit checkpoints (PSM-A4).
    ///
    /// Operates on the persistent `audit_checkpoints` SQLite table
    /// (V007). This is **mock** anchoring, NOT real onchain
    /// anchoring — the `mock_anchor_ref` is a deterministic local id
    /// derived from the checkpoint content; nothing is broadcast and
    /// nothing is signed by any chain. Every CLI line carries a
    /// `mock-anchor:` prefix for loud disclosure. See
    /// `docs/cli/audit-checkpoint.md`.
    Checkpoint {
        #[command(subcommand)]
        op: CheckpointCmd,
    },
    /// Build the ENS `setText(sbo3l:audit_root, ...)` envelope that
    /// would write the current audit chain digest into an ENS Public
    /// Resolver text record. **Dry-run by default** — no network, no
    /// signing. `--offline-fixture` writes the same envelope to disk
    /// for demo / CI fixture use. `--broadcast` is gated and emits an
    /// honest "not implemented in this build" error pointing the
    /// operator at the dry-run for the same content. See B3.
    AnchorEns {
        /// SBO3L SQLite database path. The chain digest is computed
        /// over every event_hash in the chain prefix, in seq order.
        #[arg(long)]
        db: PathBuf,
        /// ENS domain whose `sbo3l:audit_root` text record will be
        /// written. The CLI does not normalise (ENSIP-15 / UTS-46);
        /// supply an already-normalised name.
        #[arg(long)]
        domain: String,
        /// Network: `mainnet` or `sepolia`. Determines which Public
        /// Resolver address the dry-run targets when `--resolver` is
        /// not supplied.
        #[arg(long, default_value = "sepolia")]
        network: String,
        /// Override the resolver contract address. Default: the
        /// network's well-known ENS Public Resolver.
        #[arg(long)]
        resolver: Option<String>,
        /// Send the tx for real. Currently emits an honest
        /// "not implemented in this build" error.
        #[arg(long)]
        broadcast: bool,
        /// JSON-RPC endpoint to broadcast through (only consulted in
        /// `--broadcast` mode, which is currently gated).
        #[arg(long)]
        rpc_url: Option<String>,
        /// Name of the env var holding the operator's signing key
        /// (only consulted in `--broadcast` mode, which is currently
        /// gated). The key itself is read at runtime, never logged.
        #[arg(long, default_value = "SBO3L_ANCHOR_KEY")]
        private_key_env_var: String,
        /// Write the dry-run envelope to a fixture path. Default:
        /// `demo-fixtures/mock-ens-anchor.json`. When supplied, the
        /// envelope is written to that path *in addition* to being
        /// printed.
        #[arg(long, num_args = 0..=1, default_missing_value = audit_anchor_ens::DEFAULT_OFFLINE_FIXTURE)]
        offline_fixture: Option<PathBuf>,
        /// Optional dry-run-only output path. When supplied without
        /// `--offline-fixture`, the dry-run envelope is also written
        /// to this path.
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// On-chain audit-root anchor.
    ///
    /// Computes a 32-byte digest over the local audit chain head +
    /// ABI-encodes a `publishAnchor(bytes32 tenantId, bytes32
    /// auditRoot, uint64 chainHeadBlock)` call against Dev 4's
    /// AnchorRegistry contract.
    ///
    /// `--dry-run` (default) prints the envelope. `--broadcast`
    /// signs + sends the tx via alloy (requires `--features
    /// eth_broadcast` at build time + a funded signer key).
    Anchor {
        /// SBO3L SQLite database path. The chain digest is computed
        /// over the audit-chain head (latest event in the per-tenant
        /// subsequence).
        #[arg(long)]
        db: PathBuf,
        /// Tenant id, hex-encoded with optional `0x` prefix (32
        /// bytes / 64 hex chars). Defaults to `keccak256("default")`
        /// for single-tenant deployments.
        #[arg(long)]
        tenant_id: Option<String>,
        /// `mainnet` | `sepolia`. Default `sepolia`. Mainnet
        /// additionally requires `SBO3L_ALLOW_MAINNET_TX=1` in env.
        #[arg(long, default_value = "sepolia")]
        network: String,
        /// Override the AnchorRegistry contract address. Default:
        /// the network's well-known deployment (`0x0000…0000` until
        /// Dev 4's deployment pins a real address).
        #[arg(long)]
        registry: Option<String>,
        /// EVM block number the digest is being anchored against.
        /// Surfaces in the on-chain `AnchorPublished` event.
        /// Operators running the cron job typically pass
        /// `eth_blockNumber` from the RPC at job start.
        #[arg(long, default_value_t = 0)]
        chain_head_block: u64,
        /// Send the tx for real (otherwise dry-run only).
        #[arg(long)]
        broadcast: bool,
        /// JSON-RPC endpoint (only consulted with `--broadcast`).
        #[arg(long)]
        rpc_url: Option<String>,
        /// Name of the env var holding the operator's signing key.
        /// Default `SBO3L_DEPLOYER_PRIVATE_KEY` (matches the GH
        /// Actions secret name).
        #[arg(long)]
        private_key_env_var: Option<String>,
        /// Write the envelope to `<path>` as JSON in addition to
        /// printing.
        #[arg(long)]
        out: Option<PathBuf>,
    },

    /// Read-side mirror of `Anchor` — fetches an Ethereum tx by
    /// hash, decodes its `publishAnchor` calldata, and asserts the
    /// on-chain `auditRoot` matches the local audit chain head's
    /// recomputed digest.
    ///
    /// No private keys, no broadcast. Safe to run from a judge's
    /// terminal against any public Sepolia RPC.
    VerifyAnchor {
        /// 0x-prefixed Ethereum tx hash (66 chars including 0x).
        tx_hash: String,
        /// `mainnet` | `sepolia`. Default `sepolia`.
        #[arg(long, default_value = "sepolia")]
        network: String,
        /// Local SBO3L SQLite DB to recompute the audit root from.
        #[arg(long)]
        db: PathBuf,
        /// JSON-RPC endpoint. Falls back to SBO3L_RPC_URL env, else
        /// a public PublicNode endpoint for the network.
        #[arg(long)]
        rpc_url: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum CheckpointCmd {
    /// Create a checkpoint from the current audit chain tip.
    /// Writes one row to `audit_checkpoints` and prints the
    /// resulting artifact. With `--out <file>`, the same JSON is
    /// also written to disk for offline distribution.
    Create {
        /// SBO3L SQLite database path.
        #[arg(long)]
        db: PathBuf,
        /// Optional output path for the checkpoint JSON artifact.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Verify a checkpoint JSON artifact. Structural checks always
    /// run; with `--db <path>`, the chain digest is also re-derived
    /// from the live chain and the row is looked up by anchor ref.
    Verify {
        /// Path to a checkpoint JSON file produced by
        /// `sbo3l audit checkpoint create`.
        path: PathBuf,
        /// SBO3L SQLite database path. When supplied, the verify
        /// step also confirms the checkpoint was issued by *this* DB
        /// and that the live chain still anchors back to it.
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum AprpCmd {
    /// Validate an APRP JSON document against schemas/aprp_v1.json
    Validate {
        /// Path to the APRP JSON file
        path: PathBuf,
    },
    /// Compute the canonical SHA-256 request hash of an APRP document
    Hash {
        /// Path to the APRP JSON file
        path: PathBuf,
    },
    /// Validate every APRP fixture under test-corpus/ and report pass/fail
    RunCorpus {
        /// Path to the test-corpus directory (defaults to ./test-corpus)
        #[arg(long, default_value = "test-corpus")]
        root: PathBuf,
    },
}

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Aprp {
            op: AprpCmd::Validate { path },
        } => match cmd_aprp_validate(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(rc) => rc,
        },
        Command::Aprp {
            op: AprpCmd::Hash { path },
        } => match cmd_aprp_hash(&path) {
            Ok(()) => ExitCode::SUCCESS,
            Err(rc) => rc,
        },
        Command::Aprp {
            op: AprpCmd::RunCorpus { root },
        } => cmd_aprp_corpus(&root),
        Command::VerifyAudit {
            path,
            skip_hash,
            pubkey,
        } => cmd_verify_audit(&path, !skip_hash, pubkey.as_deref()),
        Command::Schema { kind } => cmd_schema(&kind),
        Command::Audit {
            op:
                AuditCmd::Export {
                    receipt,
                    chain,
                    db,
                    receipt_pubkey,
                    audit_pubkey,
                    out,
                    backend,
                    zerog_indexer_url,
                },
        } => cmd_audit_export(
            &receipt,
            chain.as_deref(),
            db.as_deref(),
            &receipt_pubkey,
            &audit_pubkey,
            out.as_deref(),
            &backend,
            zerog_indexer_url.as_deref(),
        ),
        Command::Audit {
            op: AuditCmd::VerifyBundle { path },
        } => cmd_audit_verify_bundle(&path),
        Command::Audit {
            op:
                AuditCmd::Checkpoint {
                    op: CheckpointCmd::Create { db, out },
                },
        } => audit_checkpoint::cmd_create(&db, out.as_deref()),
        Command::Audit {
            op:
                AuditCmd::Checkpoint {
                    op: CheckpointCmd::Verify { path, db },
                },
        } => audit_checkpoint::cmd_verify(&path, db.as_deref()),
        Command::Audit {
            op:
                AuditCmd::AnchorEns {
                    db,
                    domain,
                    network,
                    resolver,
                    broadcast,
                    rpc_url,
                    private_key_env_var,
                    offline_fixture,
                    out,
                },
        } => audit_anchor_ens::cmd_anchor_ens(audit_anchor_ens::AnchorEnsArgs {
            db,
            domain,
            network,
            resolver,
            broadcast,
            rpc_url,
            private_key_env_var: Some(private_key_env_var),
            offline_fixture,
            out,
        }),
        Command::Audit {
            op:
                AuditCmd::Anchor {
                    db,
                    tenant_id,
                    network,
                    registry,
                    chain_head_block,
                    broadcast,
                    rpc_url,
                    private_key_env_var,
                    out,
                },
        } => audit_anchor::cmd_audit_anchor(audit_anchor::AuditAnchorArgs {
            db,
            tenant_id,
            network,
            registry,
            chain_head_block,
            broadcast,
            rpc_url,
            private_key_env_var,
            out,
        }),
        Command::Audit {
            op:
                AuditCmd::VerifyAnchor {
                    tx_hash,
                    network,
                    db,
                    rpc_url,
                },
        } => audit_verify_anchor::cmd_audit_verify_anchor(audit_verify_anchor::VerifyAnchorArgs {
            tx_hash,
            network,
            db,
            rpc_url,
        }),
        Command::Doctor {
            db,
            json,
            extended,
            rpc_url,
        } => doctor::run_with_extended(db.as_deref(), json, extended, rpc_url.as_deref()),
        Command::Key {
            op:
                KeyCmd::Init {
                    mock,
                    role,
                    root_seed,
                    genesis,
                    db,
                },
        } => key::cmd_init(mock, &role, &root_seed, genesis.as_deref(), &db),
        Command::Key {
            op: KeyCmd::List { mock, role, db },
        } => key::cmd_list(mock, role.as_deref(), &db),
        Command::Key {
            op:
                KeyCmd::Rotate {
                    mock,
                    role,
                    root_seed,
                    db,
                },
        } => key::cmd_rotate(mock, &role, &root_seed, &db),
        Command::Policy {
            op: PolicyCmd::Validate { path },
        } => policy::cmd_validate(&path),
        Command::Policy {
            op: PolicyCmd::Current { db },
        } => policy::cmd_current(&db),
        Command::Policy {
            op: PolicyCmd::Activate { path, db, source },
        } => policy::cmd_activate(&path, &db, source.as_deref()),
        Command::Policy {
            op: PolicyCmd::Diff { a, b },
        } => policy::cmd_diff(&a, &b),
        Command::Passport {
            op:
                PassportCmd::Verify {
                    path,
                    strict,
                    receipt_pubkey,
                    audit_bundle,
                    policy,
                },
        } => passport::cmd_verify(passport::VerifyArgs {
            path,
            strict,
            receipt_pubkey,
            audit_bundle,
            policy,
        }),
        Command::Passport {
            op:
                PassportCmd::Run {
                    aprp,
                    db,
                    agent,
                    resolver,
                    ens_fixture,
                    executor,
                    mode,
                    out,
                    schema_version,
                },
        } => passport::cmd_run(passport::RunArgs {
            aprp_path: aprp,
            db_path: db,
            agent,
            resolver: match resolver {
                ResolverChoiceArg::OfflineFixture => passport::ResolverChoice::OfflineFixture,
                ResolverChoiceArg::LiveEns => passport::ResolverChoice::LiveEns,
            },
            ens_fixture,
            executor: match executor {
                ExecutorChoiceArg::Keeperhub => passport::ExecutorChoice::Keeperhub,
                ExecutorChoiceArg::Uniswap => passport::ExecutorChoice::Uniswap,
            },
            mode: match mode {
                ModeChoiceArg::Mock => passport::ModeChoice::Mock,
                ModeChoiceArg::Live => passport::ModeChoice::Live,
            },
            out_path: out,
            schema_version: match schema_version {
                SchemaVersionArg::V1 => passport::SchemaVersionChoice::V1,
                SchemaVersionArg::V2 => passport::SchemaVersionChoice::V2,
            },
        }),
        Command::Passport {
            op: PassportCmd::Explain { path, json },
        } => passport::cmd_explain(&path, json),
        Command::Agent {
            op:
                AgentCmd::Register {
                    name,
                    parent,
                    network,
                    records,
                    owner,
                    resolver,
                    broadcast,
                    // `--dry-run` is acknowledged but doesn't change
                    // behaviour: dry-run is the default, broadcast
                    // is opt-in via `--broadcast`. Clap's
                    // conflicts_with already enforces the mutex; we
                    // accept the flag here so scripts that pass it
                    // for defense-in-depth aren't rejected as
                    // "unknown argument".
                    dry_run: _,
                    rpc_url,
                    private_key_env_var,
                    out,
                },
        } => agent::cmd_agent_register(agent::AgentRegisterArgs {
            name,
            parent,
            network,
            records_json: records,
            owner,
            resolver,
            broadcast,
            rpc_url,
            private_key_env_var,
            out,
        }),
        Command::Agent {
            op:
                AgentCmd::VerifyEns {
                    fqdn,
                    network,
                    rpc_url,
                    expected_pubkey,
                    key_file,
                    expected_records,
                    lenient,
                    json,
                },
        } => agent_verify::cmd_agent_verify_ens(agent_verify::AgentVerifyEnsArgs {
            fqdn,
            network,
            rpc_url,
            expected_pubkey,
            key_file,
            lenient,
            expected_records,
            json,
        }),
        Command::Agent {
            op:
                AgentCmd::ReputationPublish {
                    fqdn,
                    events,
                    network,
                    resolver,
                    out,
                    broadcast,
                    rpc_url,
                    private_key_env_var,
                    multi_chain,
                },
        } => agent_reputation::cmd_agent_reputation_publish(
            agent_reputation::ReputationPublishArgs {
                fqdn,
                events,
                network,
                resolver,
                out,
                // --multi-chain implies --broadcast (a multi-chain
                // dry-run wouldn't add anything beyond the single-
                // chain dry-run since the calldata doesn't change
                // per-chain at the setText path).
                broadcast: broadcast || multi_chain.is_some(),
                rpc_url,
                private_key_env_var,
                multi_chain,
            },
        ),
        Command::Agent {
            op: AgentCmd::ReputationAggregate { input, out },
        } => agent_reputation_aggregate::cmd_agent_reputation_aggregate(
            agent_reputation_aggregate::ReputationAggregateArgs { input, out },
        ),
        Command::Admin {
            op:
                AdminCmd::Backup {
                    db,
                    to,
                    encrypt_with,
                },
        } => admin_backup::cmd_backup(admin_backup::BackupArgs {
            db,
            to,
            encrypt_with,
        }),
        Command::Admin {
            op:
                AdminCmd::Restore {
                    from,
                    db,
                    decrypt_with,
                },
        } => admin_backup::cmd_restore(admin_backup::RestoreArgs {
            from,
            db,
            decrypt_with,
        }),
        Command::Admin {
            op: AdminCmd::Export { db, to, format },
        } => admin_backup::cmd_export(admin_backup::ExportArgs { db, to, format }),
        Command::Admin {
            op: AdminCmd::Verify { from, decrypt_with },
        } => admin_backup::cmd_verify(admin_backup::VerifyArgs { from, decrypt_with }),
    }
}

fn cmd_aprp_validate(path: &Path) -> Result<(), ExitCode> {
    let value = read_json(path).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })?;
    match schema::validate_aprp(&value) {
        Ok(()) => {
            println!("ok: {}", path.display());
            Ok(())
        }
        Err(err) => {
            eprintln!("invalid: {} -> {} ({err})", path.display(), err.code());
            Err(ExitCode::from(1))
        }
    }
}

fn cmd_aprp_hash(path: &Path) -> Result<(), ExitCode> {
    let value = read_json(path).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })?;
    let h = sbo3l_core::hashing::request_hash(&value).map_err(|e| {
        eprintln!("error: {e}");
        ExitCode::from(2)
    })?;
    println!("{h}");
    Ok(())
}

#[derive(Debug)]
struct CorpusCase {
    relative: &'static str,
    expect_valid: bool,
    expect_code: Option<&'static str>,
}

const APRP_CORPUS: &[CorpusCase] = &[
    CorpusCase {
        relative: "aprp/golden_001_minimal.json",
        expect_valid: true,
        expect_code: None,
    },
    CorpusCase {
        relative: "aprp/deny_prompt_injection_request.json",
        expect_valid: true,
        expect_code: None,
    },
    CorpusCase {
        relative: "aprp/adversarial_unknown_field.json",
        expect_valid: false,
        expect_code: Some("schema.unknown_field"),
    },
];

fn cmd_aprp_corpus(root: &Path) -> ExitCode {
    let mut all_ok = true;
    for case in APRP_CORPUS {
        let path = root.join(case.relative);
        match read_json(&path) {
            Ok(value) => {
                let result = schema::validate_aprp(&value);
                let actual_valid = result.is_ok();
                let actual_code = result.as_ref().err().map(SchemaError::code);
                let status_ok = actual_valid == case.expect_valid
                    && match (case.expect_code, actual_code) {
                        (None, _) => true,
                        (Some(want), Some(got)) => want == got,
                        _ => false,
                    };
                if status_ok {
                    println!(
                        "ok    {} expect_valid={} actual={} code={:?}",
                        case.relative, case.expect_valid, actual_valid, actual_code
                    );
                } else {
                    all_ok = false;
                    println!(
                        "FAIL  {} expect_valid={} expect_code={:?} actual_valid={} actual_code={:?}",
                        case.relative,
                        case.expect_valid,
                        case.expect_code,
                        actual_valid,
                        actual_code
                    );
                }
            }
            Err(e) => {
                all_ok = false;
                println!("ERROR {}: {e}", path.display());
            }
        }
    }
    if all_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn cmd_schema(kind: &str) -> ExitCode {
    let id = match kind {
        "aprp" => schema::APRP_SCHEMA_ID,
        "policy" => schema::POLICY_SCHEMA_ID,
        "x402" => schema::X402_SCHEMA_ID,
        "policy-receipt" => schema::POLICY_RECEIPT_SCHEMA_ID,
        "decision-token" => schema::DECISION_TOKEN_SCHEMA_ID,
        "audit-event" => schema::AUDIT_EVENT_SCHEMA_ID,
        other => {
            eprintln!("unknown schema kind: {other}");
            return ExitCode::from(2);
        }
    };
    println!("{id}");
    ExitCode::SUCCESS
}

fn read_json(path: &Path) -> anyhow::Result<serde_json::Value> {
    let data = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&data)?;
    Ok(value)
}

fn cmd_verify_audit(path: &Path, verify_hashes: bool, pubkey: Option<&str>) -> ExitCode {
    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let mut events: Vec<SignedAuditEvent> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let signed: SignedAuditEvent = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("invalid JSON at line {}: {e}", i + 1);
                return ExitCode::from(1);
            }
        };
        // Schema-validate too.
        let raw: serde_json::Value = serde_json::from_str(line).unwrap();
        if let Err(e) = schema::validate_audit_event(&raw) {
            eprintln!(
                "schema invalid at line {} (seq={}): {e}",
                i + 1,
                signed.event.seq
            );
            return ExitCode::from(1);
        }
        events.push(signed);
    }
    match verify_chain(&events, verify_hashes, pubkey) {
        Ok(()) => {
            println!(
                "ok: {} events verified (hashes={}, sig={})",
                events.len(),
                verify_hashes,
                pubkey.is_some()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("audit chain invalid: {e}");
            ExitCode::from(1)
        }
    }
}

fn read_audit_chain_jsonl(path: &Path) -> anyhow::Result<Vec<SignedAuditEvent>> {
    let data = std::fs::read_to_string(path)?;
    let mut events: Vec<SignedAuditEvent> = Vec::new();
    for (i, line) in data.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let signed: SignedAuditEvent = serde_json::from_str(line).map_err(|e| {
            anyhow::anyhow!("chain JSONL line {} is not a SignedAuditEvent: {e}", i + 1)
        })?;
        events.push(signed);
    }
    Ok(events)
}

/// Open a SBO3L SQLite store and slice the audit chain prefix through
/// the receipt's `audit_event_id`. Pre-flights the chain segment with
/// `verify_chain` against the supplied audit pubkey AND verifies the
/// receipt signature against the supplied receipt pubkey, so a DB-backed
/// export with mismatched keys or a corrupt chain fails immediately
/// with a clear message instead of producing an unverifiable bundle.
fn read_audit_chain_from_db(
    db_path: &Path,
    receipt: &PolicyReceipt,
    receipt_pubkey_hex: &str,
    audit_pubkey_hex: &str,
) -> anyhow::Result<Vec<SignedAuditEvent>> {
    if !db_path.exists() {
        anyhow::bail!("db path does not exist: {}", db_path.display());
    }
    let storage = sbo3l_storage::Storage::open(db_path)
        .map_err(|e| anyhow::anyhow!("opening db {}: {e}", db_path.display()))?;
    let chain = storage
        .audit_chain_prefix_through(&receipt.audit_event_id)
        .map_err(|e| anyhow::anyhow!("reading chain prefix from db: {e}"))?;
    // Pre-flight: chain integrity under the supplied audit pubkey. Catches
    // (a) a tampered DB, (b) a wrong --audit-pubkey, (c) a malformed pubkey
    // hex string — all surface here, not later in verify-bundle.
    verify_chain(&chain, true, Some(audit_pubkey_hex))
        .map_err(|e| anyhow::anyhow!("audit chain pre-flight failed: {e}"))?;
    // Pre-flight: receipt signature under the supplied receipt pubkey.
    receipt
        .verify(receipt_pubkey_hex)
        .map_err(|e| anyhow::anyhow!("receipt signature pre-flight failed: {e:?}"))?;
    Ok(chain)
}

#[allow(clippy::too_many_arguments)]
fn cmd_audit_export(
    receipt_path: &Path,
    chain_path: Option<&Path>,
    db_path: Option<&Path>,
    receipt_pubkey_hex: &str,
    audit_pubkey_hex: &str,
    out: Option<&Path>,
    backend: &str,
    zerog_indexer_url: Option<&str>,
) -> ExitCode {
    let receipt: PolicyReceipt = match std::fs::read_to_string(receipt_path)
        .map_err(anyhow::Error::from)
        .and_then(|s| serde_json::from_str(&s).map_err(anyhow::Error::from))
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error reading receipt {}: {e}", receipt_path.display());
            return ExitCode::from(2);
        }
    };
    // Clap enforces "exactly one of --chain / --db"; this match is a guard
    // against future flag rearrangements that would break that invariant.
    let chain = match (chain_path, db_path) {
        (Some(p), None) => match read_audit_chain_jsonl(p) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error reading chain {}: {e}", p.display());
                return ExitCode::from(2);
            }
        },
        (None, Some(p)) => {
            match read_audit_chain_from_db(p, &receipt, receipt_pubkey_hex, audit_pubkey_hex) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("error reading chain from db {}: {e}", p.display());
                    return ExitCode::from(1);
                }
            }
        }
        _ => {
            eprintln!("internal error: exactly one of --chain or --db must be supplied");
            return ExitCode::from(2);
        }
    };
    let bundle = match audit_bundle::build(
        receipt,
        chain,
        receipt_pubkey_hex.to_string(),
        audit_pubkey_hex.to_string(),
        chrono::Utc::now(),
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error building bundle: {e}");
            return ExitCode::from(1);
        }
    };
    // Pretty-print so humans can diff bundles visually; structure is the
    // same as the compact form because field order is fixed by the derive.
    let serialised = match serde_json::to_string_pretty(&bundle) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error serialising bundle: {e}");
            return ExitCode::from(2);
        }
    };

    match backend {
        // Default path. Identical to pre-Task-C behaviour: write the bundle
        // JSON to `--out` (or stdout). No remote upload, no live_evidence.
        "local" => {
            match out {
                Some(p) => {
                    if let Err(e) = std::fs::write(p, serialised.as_bytes()) {
                        eprintln!("error writing {}: {e}", p.display());
                        return ExitCode::from(2);
                    }
                    eprintln!(
                        "wrote bundle to {} (chain length: {}, audit_event_id: {})",
                        p.display(),
                        bundle.audit_chain_segment.len(),
                        bundle.audit_event.event.id
                    );
                }
                None => {
                    println!("{serialised}");
                }
            }
            ExitCode::SUCCESS
        }
        // 0G Storage upload path. Builds an "export envelope" wrapper
        // containing the (unmodified) bundle plus a `live_evidence` block
        // recording the upload. The bundle JSON itself is what gets sent
        // to 0G — so anyone who fetches the rootHash gets a directly
        // re-verifiable AuditBundle, not an envelope they have to unwrap.
        //
        // Writing an envelope (rather than mutating AuditBundle to carry
        // a `live_evidence` field) preserves AuditBundle v1's
        // `deny_unknown_fields` schema invariant. See PR body for the
        // explicit discrepancy note.
        "0g-storage" => {
            use sbo3l_storage::zerog_backend::{
                RemoteBackend, ZeroGStorageBackend, DEFAULT_ZEROG_INDEXER_URL,
            };
            let endpoint = zerog_indexer_url
                .map(|s| s.to_string())
                .or_else(|| std::env::var("SBO3L_ZEROG_INDEXER_URL").ok())
                .unwrap_or_else(|| DEFAULT_ZEROG_INDEXER_URL.to_string());
            let zerog = ZeroGStorageBackend::new(&endpoint);
            eprintln!(
                "uploading bundle to 0G Storage testnet (indexer: {endpoint}; \
                 max attempts: {})",
                zerog.max_attempts()
            );
            let remote_ref = match zerog.upload(serialised.as_bytes()) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("0g-storage upload failed: {e}");
                    return ExitCode::from(1);
                }
            };
            // Envelope = { bundle, live_evidence }. The bundle is bit-for-bit
            // what would have been written with --backend local.
            let envelope = serde_json::json!({
                "bundle": &bundle,
                "live_evidence": {
                    "backend": remote_ref.backend,
                    "root_hash": remote_ref.root_hash,
                    "uploaded_at": remote_ref.uploaded_at.to_rfc3339(),
                    "indexer_url": remote_ref.endpoint,
                },
            });
            let envelope_str = match serde_json::to_string_pretty(&envelope) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("error serialising envelope: {e}");
                    return ExitCode::from(2);
                }
            };
            match out {
                Some(p) => {
                    if let Err(e) = std::fs::write(p, envelope_str.as_bytes()) {
                        eprintln!("error writing {}: {e}", p.display());
                        return ExitCode::from(2);
                    }
                    eprintln!(
                        "wrote envelope to {} (chain length: {}, audit_event_id: {})",
                        p.display(),
                        bundle.audit_chain_segment.len(),
                        bundle.audit_event.event.id
                    );
                }
                None => {
                    println!("{envelope_str}");
                }
            }
            // Print rootHash on its own line so shell pipelines can capture
            // it without parsing the envelope.
            println!("rootHash={}", remote_ref.root_hash);
            ExitCode::SUCCESS
        }
        other => {
            eprintln!(
                "unknown backend '{other}' (clap should have rejected this); \
                 expected 'local' or '0g-storage'"
            );
            ExitCode::from(2)
        }
    }
}

fn cmd_audit_verify_bundle(path: &Path) -> ExitCode {
    let data = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error reading {}: {e}", path.display());
            return ExitCode::from(2);
        }
    };
    let bundle: AuditBundle = match serde_json::from_str(&data) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("invalid bundle JSON: {e}");
            return ExitCode::from(2);
        }
    };
    match audit_bundle::verify(&bundle) {
        Ok(summary) => {
            println!(
                "ok: bundle verified (decision={:?}, deny_code={:?}, chain_length={}, audit_event_id={})",
                summary.decision,
                summary.deny_code,
                summary.audit_chain_length,
                summary.audit_event_id
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("bundle invalid: {e}");
            ExitCode::from(1)
        }
    }
}
