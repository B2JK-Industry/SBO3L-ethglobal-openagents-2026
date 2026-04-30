# syntax=docker/dockerfile:1.7
#
# SBO3L server + CLI multi-stage image.
#
# Stage 1: rust:1-bookworm — builds the `sbo3l-server` daemon and the
#          `sbo3l` CLI in release mode with debug symbols stripped. Tracks
#          the latest 1.x stable; some workspace transitive deps (icu_*,
#          time-macros) demand rustc >= 1.88, so a fixed minor pin would
#          drift out of compliance over time.
# Stage 2: gcr.io/distroless/cc-debian12:nonroot — minimal runtime, glibc
#          + libgcc only, no shell, runs as UID 65532. The migrations/ tree
#          is also copied into /usr/local/share/sbo3l/migrations as a
#          belt-and-suspenders artifact (the server embeds them via
#          include_str! so the runtime copy is informational, not used).
#
# Build:    docker build -t sbo3l/server .
# Run:      docker run --rm -p 8730:8730 -e SBO3L_ALLOW_UNAUTHENTICATED=1 sbo3l/server
# Persist:  docker run --rm -p 8730:8730 -v sbo3l-data:/var/lib/sbo3l \
#               -e SBO3L_BEARER_TOKEN_HASH='$2b$...' sbo3l/server
#
# See docs/cli/docker.md for full operator notes (env vars, volume layout,
# health check, security caveats around SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1).

ARG RUST_VERSION=1
ARG DEBIAN_VERSION=bookworm
ARG DISTROLESS_TAG=nonroot

# ---------- Stage 1: builder ----------
FROM rust:${RUST_VERSION}-${DEBIAN_VERSION} AS builder

WORKDIR /build

# Strip symbols at link time so we don't depend on `strip` being present
# in the rust image (binutils availability varies across rust-debian tags).
ENV RUSTFLAGS="-C strip=symbols"
ENV CARGO_TERM_COLOR=never
ENV CARGO_NET_RETRY=5

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,target=/build/target,sharing=locked \
    cargo build --release --locked \
        --bin sbo3l-server \
        --bin sbo3l \
        --bin sbo3l-mcp && \
    install -Dm0755 target/release/sbo3l-server /out/usr/local/bin/sbo3l-server && \
    install -Dm0755 target/release/sbo3l        /out/usr/local/bin/sbo3l && \
    install -Dm0755 target/release/sbo3l-mcp    /out/usr/local/bin/sbo3l-mcp && \
    mkdir -p /out/usr/local/share/sbo3l && \
    cp -r migrations /out/usr/local/share/sbo3l/migrations && \
    install -d -m 0700 -o 65532 -g 65532 /out/var/lib/sbo3l

# ---------- Stage 2: runtime ----------
FROM gcr.io/distroless/cc-debian12:${DISTROLESS_TAG}

LABEL org.opencontainers.image.title="sbo3l-server" \
      org.opencontainers.image.description="SBO3L cryptographically verifiable trust layer for autonomous AI agents" \
      org.opencontainers.image.source="https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026" \
      org.opencontainers.image.licenses="Apache-2.0"

# Binaries + embedded migrations + writable data dir.
# `sbo3l-mcp` ships in the same image so docker-compose can spin up a stdio
# JSON-RPC MCP server (profile `mcp`) without a second build.
COPY --from=builder /out/usr/local/bin/sbo3l-server     /usr/local/bin/sbo3l-server
COPY --from=builder /out/usr/local/bin/sbo3l            /usr/local/bin/sbo3l
COPY --from=builder /out/usr/local/bin/sbo3l-mcp        /usr/local/bin/sbo3l-mcp
COPY --from=builder /out/usr/local/share/sbo3l          /usr/local/share/sbo3l
COPY --from=builder --chown=nonroot:nonroot /out/var/lib/sbo3l /var/lib/sbo3l

# In a container, public bind is the entire point. Both env vars are set so
# `docker run` works with no extra flags. Unauthenticated mode is NOT set
# here — operators opt in explicitly per environment.
ENV SBO3L_LISTEN=0.0.0.0:8730 \
    SBO3L_ALLOW_UNSAFE_PUBLIC_BIND=1 \
    SBO3L_DB=/var/lib/sbo3l/sbo3l.db

VOLUME ["/var/lib/sbo3l"]

WORKDIR /var/lib/sbo3l
USER nonroot:nonroot

EXPOSE 8730

ENTRYPOINT ["/usr/local/bin/sbo3l-server"]
