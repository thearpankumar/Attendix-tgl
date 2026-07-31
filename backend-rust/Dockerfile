# ==============================================================================
# Stage 1: Rust Builder & Model Downloader
# ==============================================================================
FROM rust:1.97-slim AS builder

WORKDIR /usr/src/app

RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

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
FROM debian:trixie-slim

WORKDIR /app

RUN echo 'Acquire::ForceIPv4 "true";' > /etc/apt/apt.conf.d/99force-ipv4 && \
    apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 libstdc++6 curl && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/attendance-geotag-backend /usr/local/bin/

RUN useradd -m appuser && chown -R appuser:appuser /app

COPY --from=builder --chown=appuser:appuser /root/.cache /home/appuser/.cache

USER appuser

EXPOSE 5000

CMD ["attendance-geotag-backend"]
