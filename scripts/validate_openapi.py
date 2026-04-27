#!/usr/bin/env python3
"""Validate docs/api/openapi.json with the correct base URI.

The OpenAPI spec uses relative external refs like
``"$ref": "../../schemas/aprp_v1.json"`` which the validator can only resolve
when invoked with a base URI pointing at the spec file. ``read_from_filename``
returns that URI for us.
"""
from __future__ import annotations

import sys
from pathlib import Path


def main() -> int:
    repo_root = Path(__file__).resolve().parent.parent
    spec_path = repo_root / "docs" / "api" / "openapi.json"
    if not spec_path.is_file():
        print(f"error: spec not found at {spec_path}", file=sys.stderr)
        return 2

    try:
        from openapi_spec_validator import validate
        from openapi_spec_validator.readers import read_from_filename
    except ImportError as exc:  # pragma: no cover - user-facing
        print(
            "error: missing dependency openapi-spec-validator. "
            "Install with: pip install 'openapi-spec-validator>=0.7'",
            file=sys.stderr,
        )
        print(f"  detail: {exc}", file=sys.stderr)
        return 2

    spec_dict, spec_uri = read_from_filename(str(spec_path))
    try:
        validate(spec_dict, base_uri=spec_uri)
    except Exception as exc:  # pragma: no cover - user-facing
        print(f"openapi: INVALID at {spec_path}", file=sys.stderr)
        print(f"  {exc}", file=sys.stderr)
        return 1

    print(f"openapi: ok ({spec_path.relative_to(repo_root)})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
