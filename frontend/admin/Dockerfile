# Stage 1: Build
FROM node:24-slim AS builder
ARG VITE_BASE_PATH=/
ENV VITE_BASE_PATH=$VITE_BASE_PATH

WORKDIR /app
COPY package*.json ./

# Use Docker BuildKit cache mounts to speed up dependency installation
RUN --mount=type=cache,target=/root/.npm \
    npm ci

COPY . .
RUN npm run build

# Stage 2: Runtime
# nginx-unprivileged runs as the `nginx` user from the start (listens on
# 8080, not 80) instead of starting as root and relying on
# CHOWN/DAC_OVERRIDE/SETUID/SETGID/NET_BIND_SERVICE capabilities at the
# compose layer to drop privilege — see nginx.conf's `listen 8080;` and the
# matching compose port/capability changes.
FROM nginxinc/nginx-unprivileged:alpine

# Patch the base image's Alpine packages at build time rather than waiting
# on an upstream image rebuild (same rationale as the other Dockerfiles'
# apt-get upgrade step). Needs root; the base image itself already sets
# USER nginx (uid 101) by default, restored below.
#
# CACHEBUST changes on every CI build (see deploy-production.yml) so this
# upgrade always re-fetches the latest published patches instead of reusing
# a stale GHA layer cache — Trivy scans the result and fails the deploy on
# any fixed-but-unpatched CVE. Defaults to a constant for local builds.
ARG CACHEBUST=local
USER root
RUN echo "cachebust ${CACHEBUST}" && apk update && apk upgrade --no-cache
USER nginx

COPY --from=builder /app/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 8080
# 127.0.0.1, not localhost: the container is read_only, so nginx's
# ipv6-enable entrypoint script can't patch the config (fails silently on
# the read-only fs) and nginx ends up IPv4-only. /etc/hosts resolves
# localhost to ::1 first, so wget hit "connection refused" every time
# despite nginx serving fine on IPv4 — a permanently-false "unhealthy".
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1:8080/ || exit 1
CMD ["nginx", "-g", "daemon off;"]
