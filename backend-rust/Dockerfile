# ==============================================================================
# Stage 1: Rust Builder & Model Downloader
# ==============================================================================
FROM rust:1.97-slim AS builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src
# sqlx::migrate! embeds these files into the binary at compile time, so they
# must be present in the builder stage even though the final image never
# reads this directory at runtime. Two separate migration sets — main DB and
# the dedicated TimescaleDB instance (see main.rs's two migrate! calls) —
# both need to be present, or the build fails canonicalizing the missing one.
COPY migrations ./migrations
COPY migrations_timescale ./migrations_timescale

# Compile application binary inside Docker environment (guarantees GLIBC compatibility)
RUN RUSTFLAGS="-C link-arg=-lstdc++" cargo build --release

# Download and cache Hugging Face models for FaceAnalyzer
RUN cargo new --bin model-downloader && \
    cd model-downloader && \
    cargo add face_id@0.4 && cargo add tokio -F rt-multi-thread && \
    echo 'fn main() {\n\
    println!("Downloading models...");\n\
    tokio::runtime::Builder::new_current_thread()\n\
        .enable_all()\n\
        .build()\n\
        .unwrap()\n\
        .block_on(async {\n\
            face_id::analyzer::FaceAnalyzer::from_hf().build().await.expect("Failed to build face analyzer and download models");\n\
        });\n\
    println!("Done downloading models.");\n\
}' > src/main.rs && \
    RUSTFLAGS="-C link-arg=-lstdc++" cargo run --release

# ==============================================================================
# Stage 2: Final Runtime Image
# ==============================================================================
FROM ubuntu:24.04

WORKDIR /app

RUN echo 'Acquire::ForceIPv4 "true";' > /etc/apt/apt.conf.d/99force-ipv4 && \
    apt-get update && \
    apt-get upgrade -y && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 libstdc++6 curl && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/attendance-geotag-backend /usr/local/bin/

RUN useradd -m appuser && chown -R appuser:appuser /app

# Baked in at build time as .cache-seed, not .cache directly: the ONNX face
# model's runtime loader (face_id::FaceAnalyzer::from_hf) makes a HEAD
# request against huggingface.co on first use in every fresh process and,
# even on a 304 Not Modified, unconditionally rewrites its `refs/main`
# pointer file and re-creates a cache symlink — real filesystem writes, not
# just reads, on every container start. Under a read-only root filesystem
# (see docker-compose.yml) that write fails and the app silently falls back
# to weaker non-ML face detection. entrypoint.sh seeds a tmpfs-mounted
# .cache from this read-only copy before the app starts, so those writes
# land on writable tmpfs instead.
COPY --from=builder --chown=appuser:appuser /root/.cache /home/appuser/.cache-seed
RUN mkdir -p /home/appuser/.cache && chown appuser:appuser /home/appuser/.cache

RUN printf '#!/bin/sh\nset -e\nif [ -z "$(ls -A /home/appuser/.cache 2>/dev/null)" ]; then\n    cp -r /home/appuser/.cache-seed/. /home/appuser/.cache/\nfi\nexec attendance-geotag-backend\n' \
    > /usr/local/bin/entrypoint.sh && chmod +x /usr/local/bin/entrypoint.sh

USER appuser

EXPOSE 5000

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
