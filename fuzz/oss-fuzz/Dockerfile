FROM gcr.io/oss-fuzz-base/base-builder-rust

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libssl-dev \
 && rm -rf /var/lib/apt/lists/*

RUN git clone https://github.com/B2JK-Industry/SBO3L-ethglobal-openagents-2026 sbo3l

WORKDIR $SRC/sbo3l
COPY build.sh $SRC/
