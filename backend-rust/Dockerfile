# Stage 1: Build
FROM rust:1.97-slim-bookworm AS builder

WORKDIR /usr/src/app

# Install build dependencies
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies and cache them
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Use Docker BuildKit cache mounts to speed up dependency compilation
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release

RUN rm -rf src && rm -f target/release/deps/attendance_geotag_backend*

# Copy the actual source code
COPY src src

# Build the final binary utilizing the cache
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# Install necessary runtime dependencies (ca-certificates for HTTPS/AWS, libssl for crypto)
# Using --no-install-recommends is a best practice to keep the image size minimal
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl3 && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/app/target/release/attendance-geotag-backend /usr/local/bin/

# Set the binary to run as an unprivileged user for security (optional but recommended)
RUN useradd -m appuser && chown -R appuser /app
USER appuser

# Default port
EXPOSE 3000

# Start the application
CMD ["attendance-geotag-backend"]
