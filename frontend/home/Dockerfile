# Stage 1: Build
FROM oven/bun:1 AS builder
WORKDIR /app
COPY package.json bun.lock ./

# Cache bun install to speed up builds
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install

COPY . .
RUN bun run build

# Stage 2: Runtime
# oven/bun:1-slim is the optimal lightweight image for Bun
FROM oven/bun:1-slim

WORKDIR /app
COPY --from=builder /app/.output ./

# oven/bun:1-slim's Debian base lags behind Debian's own security updates
# (Trivy caught HIGH-severity CVEs in libcap2/libssl3t64 with fixed versions
# already published upstream) — pull those in explicitly rather than waiting
# on an upstream image rebuild.
#
# CACHEBUST changes on every CI build (see deploy-production.yml) so this
# upgrade always re-fetches the latest published patches instead of reusing
# a stale GHA layer cache — Trivy scans the result and fails the deploy on
# any fixed-but-unpatched CVE. Defaults to a constant for local builds.
ARG CACHEBUST=local
RUN echo "cachebust ${CACHEBUST}" && \
    apt-get update && apt-get upgrade -y && rm -rf /var/lib/apt/lists/*

# Set the binary to run as an unprivileged user for security
RUN useradd -m appuser && chown -R appuser /app
USER appuser

ENV PORT=3000
EXPOSE 3000
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD bun -e "fetch('http://localhost:3000/').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"
CMD ["bun", "run", "server/index.mjs"]
