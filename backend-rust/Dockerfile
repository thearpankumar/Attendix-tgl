# ==============================================================================
# Stage 1: Model Downloader
# ==============================================================================
FROM rust:1.97-slim AS model-builder

WORKDIR /builder
RUN apt-get update && apt-get install -y pkg-config libssl-dev build-essential && rm -rf /var/lib/apt/lists/*

RUN cargo new model-downloader
WORKDIR /builder/model-downloader
RUN cargo add face_id@0.4 && cargo add tokio -F rt-multi-thread

RUN echo 'fn main() {\n\
    println!("Downloading models...");\n\
    tokio::runtime::Builder::new_current_thread()\n\
        .enable_all()\n\
        .build()\n\
        .unwrap()\n\
        .block_on(async {\n\
            face_id::analyzer::FaceAnalyzer::from_hf().build().await.expect("Failed to build face analyzer and download models");\n\
        });\n\
    println!("Done downloading models.");\n\
}' > src/main.rs

RUN cargo run --release

# ==============================================================================
# Stage 2: Final Runtime Image
# ==============================================================================
FROM debian:trixie-slim

WORKDIR /app

RUN echo 'Acquire::ForceIPv4 "true";' > /etc/apt/apt.conf.d/99force-ipv4 && \
    apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 libstdc++6 curl && \
    rm -rf /var/lib/apt/lists/*

# Copy the host-compiled binary targeting GLIBC 2.31
COPY target/x86_64-unknown-linux-gnu.2.31/release/attendance-geotag-backend /usr/local/bin/

RUN useradd -m appuser && chown -R appuser:appuser /app

COPY --from=model-builder --chown=appuser:appuser /root/.cache /home/appuser/.cache

USER appuser

EXPOSE 5000

CMD ["attendance-geotag-backend"]
