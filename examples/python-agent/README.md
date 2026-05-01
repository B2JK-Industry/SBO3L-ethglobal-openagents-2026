# `examples/python-agent`

Minimal Python SBO3L agent. Mirror of `examples/typescript-agent/`.

> ⚠ **DRAFT (F-13):** depends on F-10 (`sbo3l-sdk`) merging + publishing to PyPI.
> While `sbo3l-sdk` is unpublished, the example installs the SDK from
> `../../sdks/python` via local `pip install -e`.

## Run

```bash
SBO3L_ALLOW_UNAUTHENTICATED=1 cargo run --bin sbo3l-server &

cd examples/python-agent
python3 -m venv .venv
.venv/bin/pip install -e ../../sdks/python
.venv/bin/python main.py
```

Expected output:

```
decision: allow
execution_ref: kh-01HTAWX5K3R8YV9NQB7C6P2DGS
audit_event_id: evt-01HTAWX5K3R8YV9NQB7C6P2DGR
request_hash: c0bd2fab1234567890abcdef1234567890abcdef1234567890abcdef12345678
policy_hash: e044f13c5acb792dd3109f1be3a98536168b0990e25595b3cedc131d02e666cf
```

Override endpoint with `SBO3L_ENDPOINT`. Pass a bearer token with `SBO3L_BEARER_TOKEN`.
