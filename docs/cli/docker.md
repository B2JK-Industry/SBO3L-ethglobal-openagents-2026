# Running SBO3L in Docker

**Audience:** operators deploying `sbo3l-server` as a long-lived container.

**Outcome:** in five minutes you have a `sbo3l-server` container running in
unauthenticated dev mode, and a curl from the host gets back a signed
`allow` decision against the golden APRP fixture.

## Image

The repository ships a multi-stage `Dockerfile` at the project root.

| Stage  | Base                                  | Role                                                |
|--------|---------------------------------------|-----------------------------------------------------|
| build  | `rust:1.85-bookworm`                  | `cargo build --release --locked --bin sbo3l-server --bin sbo3l` with `RUSTFLAGS="-C strip=symbols"` |
| runtime| `gcr.io/distroless/cc-debian12:nonroot` | runs as UID 65532, no shell, no package manager     |

Final image is **under 100 MB**. The runtime layer ships the
`sbo3l-server` daemon, the `sbo3l` CLI, and the `migrations/` tree (the
server already embeds them via `include_str!`; the on-disk copy at
`/usr/local/share/sbo3l/migrations` is a belt-and-suspenders artefact for
offline inspection).

## Build

```bash
docker build -t sbo3l/server .
```

## Run (dev mode, unauthenticated)

```bash
docker run --rm -p 8730:8730 \
  -e SBO3L_ALLOW_UNAUTHENTICATED=1 \
  --name sbo3l \
  sbo3l/server
```

The daemon prints a stderr banner reminding you that auth is bypassed:

```
UNSAFE PUBLIC BIND: sbo3l-server is listening on a non-loopback address 0.0.0.0:8730 …
⚠ UNAUTHENTICATED MODE — DEV ONLY ⚠
  SBO3L_ALLOW_UNAUTHENTICATED=1 is set; POST /v1/payment-requests will accept unauthenticated requests.
```

Both warnings are expected inside a container — see [Security caveats](#security-caveats).

## Smoke test

From a second shell:

```bash
curl -fsS http://localhost:8730/v1/payment-requests -X POST \
  -H "Content-Type: application/json" \
  -d @test-corpus/aprp/golden_001_minimal.json | jq -r .decision
# expect: allow
```

> The golden fixture has a fixed `expiry`. If the date has passed,
> override `expiry` and `nonce` inline:
>
> ```bash
> NONCE=$(LC_ALL=C tr -dc '0-9A-HJKMNPQRSTVWXYZ' </dev/urandom | head -c 26)
> EXPIRY=$(date -u -d '+1 hour' +%Y-%m-%dT%H:%M:%SZ)
> jq --arg e "$EXPIRY" --arg n "$NONCE" '.expiry=$e | .nonce=$n' \
>     test-corpus/aprp/golden_001_minimal.json | \
>   curl -fsS http://localhost:8730/v1/payment-requests -X POST \
>     -H "Content-Type: application/json" --data-binary @- | jq -r .decision
> ```

## Persistent SQLite storage

The default `SBO3L_DB=/var/lib/sbo3l/sbo3l.db` lives on a volume:

```bash
docker volume create sbo3l-data
docker run --rm -p 8730:8730 \
  -v sbo3l-data:/var/lib/sbo3l \
  -e SBO3L_ALLOW_UNAUTHENTICATED=1 \
  sbo3l/server
```

The volume mount-point is owned by UID `65532` (the distroless `nonroot`
user). Bind-mounting a host directory works only if the host directory is
writable by UID 65532 — fix with `chown 65532:65532 /your/host/dir` or
use a named docker volume as above.

To run with an in-memory database (no persistence, fastest startup):

```bash
docker run --rm -p 8730:8730 \
  -e SBO3L_ALLOW_UNAUTHENTICATED=1 \
  -e SBO3L_DB=:memory: \
  sbo3l/server
```

## Production: enable bearer auth

Generate a bcrypt hash of your bearer token and pass it in:

```bash
TOKEN=$(openssl rand -hex 32)
TOKEN_HASH=$(htpasswd -nBC 12 "" | tr -d ':\n' | sed 's/^/$2y$/' )
# or any tool that emits a bcrypt hash; the `bcrypt` Rust crate accepts
# $2a$, $2b$, and $2y$ variants.

docker run -d --restart unless-stopped \
  -p 8730:8730 \
  -v sbo3l-data:/var/lib/sbo3l \
  -e SBO3L_BEARER_TOKEN_HASH="$TOKEN_HASH" \
  --name sbo3l \
  sbo3l/server

curl -H "Authorization: Bearer $TOKEN" \
     http://localhost:8730/v1/payment-requests …
```

Without `SBO3L_BEARER_TOKEN_HASH` (and without the dev bypass), the server
returns `401 auth.required`.

## Environment reference

| Variable                          | Default in image          | Notes                                                                         |
|-----------------------------------|---------------------------|-------------------------------------------------------------------------------|
| `SBO3L_LISTEN`                    | `0.0.0.0:8730`            | Bind address. Image sets the public bind because in a container, public bind is the point. |
| `SBO3L_ALLOW_UNSAFE_PUBLIC_BIND`  | `1`                       | Override for the F-4 safety gate. Required while `SBO3L_LISTEN` is non-loopback. |
| `SBO3L_DB`                        | `/var/lib/sbo3l/sbo3l.db` | Override to `:memory:` for ephemeral runs.                                    |
| `SBO3L_ALLOW_UNAUTHENTICATED`     | unset (auth required)     | Set to `1` for dev mode. Logs a stderr banner.                                |
| `SBO3L_BEARER_TOKEN_HASH`         | unset                     | bcrypt hash of the production bearer token.                                   |
| `SBO3L_JWT_PUBKEY`                | unset                     | Ed25519 verifier pubkey for JWT auth (alternative to bearer).                 |

## Security caveats

1. **`SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1` is set in the image.** This is
   correct *inside the container* — without it the F-4 gate would refuse
   to start because `0.0.0.0` is non-loopback. **Do not** set this env on
   a host running the daemon directly; use the loopback default
   (`127.0.0.1:8730`) and put the daemon behind a reverse proxy.
2. **The image runs as UID 65532 (`nonroot`).** Don't add `--user root`
   unless you have a specific reason; the daemon never needs root.
3. **The daemon is the only process.** No shell, no `apt`, no
   `tini`/`dumb-init` shim. Signal handling is the responsibility of
   `tokio` and the binary; `docker stop` sends SIGTERM and the runtime
   exits cleanly.
4. **No HTTPS termination inside the container.** Run a TLS-terminating
   reverse proxy (nginx, Caddy, Cloudflare Tunnel) in front.

## Troubleshooting

- **Build fails with "could not find Cargo.lock"** — you ran from a
  subdirectory. The build context has to be the repo root.
- **Container starts then exits** — check `docker logs sbo3l`. The two
  most common causes are (a) F-4 refusing because something else has
  unset `SBO3L_ALLOW_UNSAFE_PUBLIC_BIND`, or (b) auth required and no
  token hash configured.
- **`401 auth.required` on every request** — set
  `SBO3L_ALLOW_UNAUTHENTICATED=1` (dev) or
  `SBO3L_BEARER_TOKEN_HASH=$2b$…` (prod) and pass
  `Authorization: Bearer …` on the request.
- **Bind-mount permission denied** — the daemon runs as UID 65532; fix
  the host directory perms or use a named volume.
