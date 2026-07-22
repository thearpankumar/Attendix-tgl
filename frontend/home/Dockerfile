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

# Set the binary to run as an unprivileged user for security
RUN useradd -m appuser && chown -R appuser /app
USER appuser

ENV PORT=3000
EXPOSE 3000
CMD ["bun", "run", "server/index.mjs"]
