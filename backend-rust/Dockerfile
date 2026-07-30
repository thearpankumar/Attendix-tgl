# ==============================================================================
# Stage 1: Rust Builder & Model Downloader
# ==============================================================================
FROM rust:1.97-slim AS builder

WORKDIR /usr/src/app

# Install build-essential, pkg-config, and libssl-dev for native C bindings
RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*

# Copy source code & Cargo configuration
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Compile application release binary inside Docker environment
RUN cargo build --release

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
    cargo run --release

# ==============================================================================
# Stage 2: Final Runtime Image
# ==============================================================================
FROM debian:trixie-slim

WORKDIR /app

# Install necessary runtime dependencies
RUN echo 'Acquire::ForceIPv4 "true";' > /etc/apt/apt.conf.d/99force-ipv4 && \
    apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 libstdc++6 curl && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage (guarantees GLIBC compatibility)
COPY --from=builder /usr/src/app/target/release/attendance-geotag-backend /usr/local/bin/

# Create an unprivileged user
RUN useradd -m appuser && chown -R appuser:appuser /app

# Copy the downloaded Hugging Face model cache from builder stage
COPY --from=builder --chown=appuser:appuser /root/.cache /home/appuser/.cache

USER appuser

EXPOSE 5000

CMD ["attendance-geotag-backend"]
