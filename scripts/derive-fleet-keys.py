#!/usr/bin/env python3
"""Derive deterministic Ed25519 keypairs for the SBO3L agent fleet.

Reads a YAML config listing agents (T-3-3 fleet-of-5 OR T-3-4
60-agent fleet) and emits per-agent secret seeds + public keys.
Determinism rule: keypair = SHA256(seed_doc + ":" + agent_label).

The seed_doc is a known-public string (committed in the YAML). Anyone
holding the YAML can re-derive every keypair byte-for-byte. This is
intentional — the agents in this fleet are *demonstration* agents,
not production secrets. A production fleet would use HSM-backed keys.

Usage:
    python3 scripts/derive-fleet-keys.py \\
      --config scripts/fleet-config/agents-5.yaml \\
      [--output-pubkeys scripts/fleet-config/agents-5.pubkeys.json] \\
      [--print-secrets]   # use ONLY when broadcasting; never commit

Exit codes:
    0 — derivation succeeded; pubkeys written / printed
    2 — config malformed or missing
    3 — cryptography library missing (pip install cryptography)
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path

try:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
except ImportError:
    sys.stderr.write(
        "ERROR: derive-fleet-keys.py requires the `cryptography` Python "
        "package. Install with `pip install cryptography`.\n"
    )
    sys.exit(3)

try:
    import yaml  # type: ignore[import-untyped]
except ImportError:
    sys.stderr.write(
        "ERROR: derive-fleet-keys.py requires PyYAML. "
        "Install with `pip install pyyaml`.\n"
    )
    sys.exit(3)


def derive_keypair(seed_doc: str, agent_label: str) -> tuple[str, str]:
    """Return (secret_seed_hex, pubkey_ed25519_hex) for an agent.

    The 32-byte secret seed is the SHA-256 of "<seed_doc>:<agent_label>".
    Ed25519 treats the 32-byte seed as the private key directly; the
    public key is derived via the curve.
    """
    digest = hashlib.sha256(f"{seed_doc}:{agent_label}".encode("utf-8")).digest()
    sk = Ed25519PrivateKey.from_private_bytes(digest)
    pk = sk.public_key().public_bytes_raw()
    return digest.hex(), pk.hex()


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", required=True, type=Path)
    ap.add_argument(
        "--output-pubkeys",
        type=Path,
        help="Write a {agent_label: pubkey_hex} JSON map to this path.",
    )
    ap.add_argument(
        "--print-secrets",
        action="store_true",
        help=(
            "Print secret seeds to stdout (1 per line: agent_label "
            "secret_hex pubkey_hex). DEFAULT OFF — secrets are never "
            "printed unless explicitly requested by a broadcast script."
        ),
    )
    args = ap.parse_args()

    if not args.config.exists():
        sys.stderr.write(f"ERROR: config not found: {args.config}\n")
        return 2

    cfg = yaml.safe_load(args.config.read_text())
    if not isinstance(cfg, dict):
        sys.stderr.write(f"ERROR: config root must be mapping; got {type(cfg).__name__}\n")
        return 2

    seed_doc = cfg.get("seed_doc")
    if not seed_doc or not isinstance(seed_doc, str):
        sys.stderr.write("ERROR: config must set `seed_doc` to a non-empty string\n")
        return 2

    agents = cfg.get("agents")
    if not isinstance(agents, list) or not agents:
        sys.stderr.write("ERROR: config must set `agents` to a non-empty list\n")
        return 2

    pubkeys: dict[str, str] = {}
    for entry in agents:
        if not isinstance(entry, dict):
            sys.stderr.write(f"ERROR: agent entry not a mapping: {entry!r}\n")
            return 2
        label = entry.get("label")
        if not label or not isinstance(label, str):
            sys.stderr.write(f"ERROR: agent missing `label`: {entry!r}\n")
            return 2
        seed_hex, pubkey_hex = derive_keypair(seed_doc, label)
        pubkeys[label] = pubkey_hex
        if args.print_secrets:
            print(f"{label} {seed_hex} {pubkey_hex}")

    if args.output_pubkeys:
        args.output_pubkeys.write_text(
            json.dumps(
                {
                    "schema": "sbo3l.fleet_pubkeys.v1",
                    "seed_doc": seed_doc,
                    "agents": pubkeys,
                },
                indent=2,
                sort_keys=True,
            )
            + "\n"
        )
        sys.stderr.write(f"wrote {len(pubkeys)} pubkeys to {args.output_pubkeys}\n")
    elif not args.print_secrets:
        # No output target — print pubkeys to stdout as a friendly default.
        for label, pubkey_hex in pubkeys.items():
            print(f"{label} {pubkey_hex}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
