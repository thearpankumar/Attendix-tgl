# 🚀 Deployment Guide

## Architecture Overview

```
┌─────────────────┐
│   Caddy (SSL)   │
│  Load Balancer  │
└────────┬────────┘
         │
    ┌────┼────┬────┐
    │    │    │    │
    ▼    ▼    ▼    ▼
┌──────┐ ┌──────┐ ┌──────┐
│ API1 │ │ API2 │ │ API3 │
│ Rust │ │ Rust │ │ Rust │
└──┬───┘ └──┬───┘ └──┬───┘
   │        │        │
   └────────┴────────┘
        │        │
  ┌─────┴────┐ ┌─┴────┐
  │ Postgres │ │Redis │
  │          │ │Cache │
  └────┬─────┘ └──────┘
       │
  ┌────┴─────┐
  │ pg-backup│  nightly pg_dump → S3
  └──────────┘
```

The backend is a single Rust/axum service (`backend-rust/`), run as multiple
container replicas behind Caddy — there is no PM2/process-manager layer, each
container is one process.

---

## 🛠️ Automated Setup

### Using the Setup Script

The project includes a comprehensive setup script (`setup.sh`) that automates the entire installation process:

```bash
# Make executable
chmod +x setup.sh

# Interactive menu mode (recommended for first-time setup)
./setup.sh menu

# Or use specific commands
./setup.sh check     # Check system requirements
./setup.sh install   # Install everything (Docker, Node.js, Dependencies)
./setup.sh up        # Start Docker Compose
./setup.sh down      # Stop Docker Compose
./setup.sh logs      # View logs
./setup.sh status    # Check service status
./setup.sh reset     # Reset and remove all volumes
./setup.sh help      # Show all options
```

### Command Line Options

```
./setup.sh [option]

Options:
  check       Run system checks
  install     Install all (Docker, Node.js, Dependencies)
  docker      Install Docker only
  node        Install Node.js only (via NVM)
  deps        Install project dependencies only
  env         Copy .env.example to .env
  up          Start Docker Compose
  down        Stop Docker Compose
  logs        View Docker Compose logs
  status      Show Docker Compose status
  reset       Reset Docker (remove volumes)
  test        Run backend tests
  lint        Run backend linter
  dev         Run backend dev server
  menu        Show interactive menu (default)
  help        Show help message
```

---

## 🚀 Quick Start

### Prerequisites
- Docker & Docker Compose
- 8GB RAM minimum
- 4 vCPUs recommended

### Option A: Automated Setup (Recommended)

```bash
# 1. Clone the repository
git clone <repository-url>
cd Attendence-GEOTAG-System

# 2. Run automated setup
./setup.sh install

# 3. Configure environment (edit .env with your credentials)
./setup.sh env

# 4. Start the system
./setup.sh up
```

### Option B: Manual Setup

### 1. Configure Environment

The `.env` file is already configured with:
- ✅ AWS S3 credentials
- ✅ Redis connection
- ✅ Postgres settings
- ✅ Development mode (localhost)

### 2. Start the System

```bash
# Build and start all services
docker-compose up -d --build

# Check logs
docker-compose logs -f

# Check service status
docker-compose ps
```

### 3. Access the Application

| Service | URL |
|---------|-----|
| **Admin Panel** | http://localhost/owner-of-attendix-xyz |
| **Student Page** | http://localhost/attend/\<token\> |
| **API Health** | http://localhost/health |
| **Direct Backend** | http://localhost:5000 |

---

## 📊 Services Breakdown

### Backend (multiple replicas)
- **Image**: Rust (axum), built from `backend-rust/Dockerfile`
- **Port**: 5000 (internal, 3000 in dev compose)
- **Health Check**: `/health` endpoint
- Runs `sqlx` migrations (`backend-rust/migrations/`) automatically on startup

### Postgres
- **Single instance**, `postgres:16-alpine`, data on the `postgres_data` volume
- Backed up nightly to S3 by the `pg-backup` sidecar (see **Backups** below)

### pg-backup
- Built from `scripts/backup/` (`postgres:16-alpine` + `aws-cli` + busybox `crond`)
- Runs `pg_dump` on `BACKUP_CRON_SCHEDULE` (default `0 2 * * *`, i.e. nightly at 02:00 UTC)
  and uploads the compressed dump to `s3://$AWS_S3_BUCKET/postgres-backups/`
- Local dumps older than `BACKUP_RETENTION_DAYS` (default 30) are pruned from
  the container's own scratch volume; long-term retention is controlled by an
  S3 lifecycle rule (one-time setup, see **Backups** below)

### Redis Cache
- **Image**: Redis 7 Alpine
- **Memory Limit**: 512MB
- **Eviction Policy**: allkeys-lru
- **Persistence**: AOF enabled
- **Port**: 6379

### Caddy (Reverse Proxy)
- **Automatic HTTPS**: ✅ (production mode)
- **Load Balancing**: Round-robin
- **Health Checks**: ✅
- **Ports**: 80, 443
- **Access logging**: JSON, `/var/log/caddy/access.log` (shared `caddy_logs`
  volume), watched by the `fail2ban` service below
- **Content-Security-Policy** (`Caddyfile.prod`, applies to all four
  frontends): locked down to `'self'` plus the specific exceptions each app
  actually needs. Three of those exist only for the attendance-photo flow
  (student capture/upload + admin/mentor display) and are easy to regress:
  - `script-src` needs `'wasm-unsafe-eval'` — required for
    `WebAssembly.instantiate`/`instantiateStreaming`, which is how the
    on-device MediaPipe face-detection model compiles. This is a distinct
    grant from `'unsafe-eval'` (JS `eval`), not a broader one.
  - `connect-src` needs `data:` (MediaPipe internally `fetch()`s the captured
    frame as a `data:` URI) and the S3 bucket's virtual-hosted host,
    `https://<AWS_S3_BUCKET>.s3.<AWS_REGION>.amazonaws.com` — the student app
    uploads the attendance photo with a browser-side `PUT` straight to a
    presigned S3 URL (`backend-rust/src/storage/s3.rs`), which never touches
    our own origin.
  - `img-src` needs that same S3 host — the admin/mentor panels render
    attendance photos with a plain `<img src=...>` pointing straight at the
    public S3 URL (`SessionDetail.tsx`). This is a separate directive from
    `connect-src`: img-src governs `<img>` loads, connect-src governs
    fetch/XHR — the S3 object returning 200 doesn't matter if `img-src`
    blocks the load first.
  - If `AWS_S3_BUCKET` or `AWS_REGION` changes, update this host in **both**
    `connect-src` and `img-src` in `Caddyfile.prod`, or you'll get a CSP
    violation in the browser console (not a server-side error — check the
    browser devtools Console/Network tab, not `docker compose logs`): photo
    upload breaks if only `connect-src` is stale, photo display breaks if
    only `img-src` is stale.

### fail2ban
- Watches Caddy's access log for repeated failed `/api/admin/login` and
  `/api/admin/register` attempts and bans the source IP at the host firewall
  (config in `fail2ban/jail.d`, `fail2ban/filter.d`).
- **Only effective on a real Linux Docker host.** It bans via the host's
  `iptables` `DOCKER-USER` chain (`network_mode: host` + `NET_ADMIN`/`NET_RAW`)
  — on Docker Desktop (Mac/Windows) it runs but can't reach the host firewall,
  so it's a no-op in local dev.
- This is a backstop behind the app's own per-IP rate limiter and per-account
  lockout — verify it's actually working on first production deploy:
  ```bash
  docker compose logs fail2ban          # confirm the jail loaded, no filter errors
  docker compose exec fail2ban fail2ban-client status attendix-admin-login
  ```
  If the filter regex doesn't match a real log line's field layout, the jail
  silently never bans anyone — check a live log line
  (`docker compose exec caddy tail -f /var/log/caddy/access.log`) against
  `fail2ban/filter.d/attendix-admin-login.conf` if `status` never leaves 0.

---

## 💾 Backups

### How it works

The `pg-backup` service (`scripts/backup/`) runs alongside `postgres` in
production (`docker-compose.postgres.yml`). On its cron schedule it:

1. Runs `pg_dump -Fc` (compressed, custom format) against the `postgres` service
2. Uploads the dump to `s3://$AWS_S3_BUCKET/postgres-backups/attendance_<timestamp>.dump`
   — the same bucket already used for attendance photos, under its own prefix
3. Deletes local dumps in its scratch volume older than `BACKUP_RETENTION_DAYS`

Trigger a backup manually (e.g. before a risky migration):

```bash
docker exec attendance-pg-backup /usr/local/bin/backup.sh
```

### One-time setup: S3 lifecycle rule

Nothing in this repo automates bucket lifecycle policy (there's no
Terraform/IaC here) — set it once per environment so old backups actually
expire instead of accumulating forever:

```bash
aws s3api put-bucket-lifecycle-configuration \
  --bucket "$AWS_S3_BUCKET" \
  --lifecycle-configuration '{
    "Rules": [{
      "ID": "expire-postgres-backups",
      "Filter": { "Prefix": "postgres-backups/" },
      "Status": "Enabled",
      "Expiration": { "Days": 90 }
    }]
  }'
```

### One-time setup: S3 bucket CORS (required for photo upload)

Also not automated by this repo (no Terraform/IaC here) — set it once per
bucket, or every student attendance submission fails at the photo-upload
step. The student app uploads attendance photos with a **browser-side `PUT`**
straight to a presigned S3 URL (`backend-rust/src/storage/s3.rs`
`get_upload_url`) rather than proxying the bytes through the backend. That
makes it a genuine cross-origin request from the browser's point of view, so
S3 itself — not just Caddy's CSP — has to answer the browser's CORS
preflight. A bucket with no CORS configuration returns `403` with no
`Access-Control-Allow-Origin` header on the preflight `OPTIONS`, which the
browser reports as a generic "Failed to fetch" / CORS error, never reaching
S3's actual response:

```bash
aws s3api put-bucket-cors --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" \
  --cors-configuration '{
    "CORSRules": [{
      "AllowedOrigins": ["https://your-domain.com"],
      "AllowedMethods": ["PUT"],
      "AllowedHeaders": ["content-type"],
      "MaxAgeSeconds": 3600
    }]
  }'
```

### One-time setup: S3 public read for `attendance-photos/` (required for photo viewing)

Also not automated by this repo. `get_file_url()` (`backend-rust/src/storage/s3.rs`)
builds a **plain, unsigned** `https://$AWS_S3_BUCKET.s3.$AWS_REGION.amazonaws.com/<key>`
URL and hands it straight to the admin panel's `<img src=...>`
(`frontend/admin/src/pages/SessionDetail.tsx`) — there is no presigned-GET or
backend-proxy step. A fresh bucket has AWS's default Block Public Access on
and no bucket policy, so every photo `GET` 403s and the admin session view
just shows a broken image with nothing in the server logs (check the
browser's Network tab, not `docker compose logs`).

Fix: allow public `s3:GetObject` scoped **only** to the `attendance-photos/`
prefix — `postgres-backups/` (and anything else in the bucket) must stay
blocked. This trades a small amount of exposure (anyone with an exact,
unguessable UUID-based photo URL can view that one photo, indefinitely, with
no auth) for zero backend changes; that tradeoff was chosen deliberately over
switching to presigned GET URLs, so don't "helpfully" lock the bucket back
down without re-introducing a presigned-GET/proxy read path first.

```bash
# 1. Allow bucket policies to grant public access (ACLs stay blocked — this
#    uses a bucket policy, not object ACLs)
aws s3api put-public-access-block --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" \
  --public-access-block-configuration \
  BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=false,RestrictPublicBuckets=false

# 2. Grant public read on the attendance-photos/ prefix only
aws s3api put-bucket-policy --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" --policy '{
  "Version": "2012-10-17",
  "Statement": [{
    "Sid": "PublicReadAttendancePhotos",
    "Effect": "Allow",
    "Principal": "*",
    "Action": "s3:GetObject",
    "Resource": "arn:aws:s3:::'"$AWS_S3_BUCKET"'/attendance-photos/*"
  }]
}'
```

Verify without your own AWS credentials (this is what the browser experiences):

```bash
curl -s -o /dev/null -w "%{http_code}\n" "https://$AWS_S3_BUCKET.s3.$AWS_REGION.amazonaws.com/attendance-photos/<some-existing-key>.jpg"
# expect 200
```

Replace `AllowedOrigins` with your actual `DOMAIN` (must be the exact
scheme+host the browser sends as `Origin` — no path, no trailing slash).
Only `PUT` is needed: nothing in the frontend `fetch()`s or canvas-reads a
photo URL cross-origin — the admin/mentor panels just render it in a plain
`<img src=...>`, which isn't CORS-gated. Verify with:

```bash
aws s3api get-bucket-cors --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION"
```

### Restoring from a backup

Restore is a manual, deliberate operation — not automated by this repo.

```bash
# 1. Pull the dump down from S3
aws s3 cp s3://$AWS_S3_BUCKET/postgres-backups/attendance_<timestamp>.dump ./restore.dump

# 2a. Restore into a fresh/empty database (recommended — verify before swapping over)
docker exec -i attendance-postgres createdb -U "$POSTGRES_USER" attendance_geotag_restore
docker exec -i attendance-postgres pg_restore -U "$POSTGRES_USER" -d attendance_geotag_restore < ./restore.dump

# 2b. Or restore in place, replacing existing data (destructive — stop the backend first)
docker exec -i attendance-postgres pg_restore -U "$POSTGRES_USER" -d attendance_geotag --clean --if-exists < ./restore.dump
```

---

## 🔧 Configuration

### Secrets

Every secret is generated by `scripts/generate-secrets.sh`. Nothing in this
repo ships a usable default: the backend refuses to boot on a missing, short
(<32 char) or placeholder secret, so `cp .env.example .env` alone will not
start the stack.

```bash
./scripts/generate-secrets.sh            # fill in whatever is missing or still a placeholder
./scripts/generate-secrets.sh --check    # verify only; exits 1 if anything is unsafe (use in CI)
./scripts/generate-secrets.sh --rotate   # regenerate everything
./scripts/generate-secrets.sh --print    # print to stdout, write nothing (for a secrets manager)
```

Running it with no arguments is safe and idempotent — existing healthy secrets
are left alone, so it can be re-run any time to fill a newly added variable.
`setup.sh` calls it automatically, and refuses to start the stack if
`--check` fails.

It generates:

| Variable | Length | Purpose |
|---|---|---|
| `JWT_SECRET` | 48 | Signs admin JWTs |
| `ADMIN_SECRET` | 48 | Gates the public `POST /api/admin/register` |
| `METRICS_TOKEN` | 48 | Prometheus bearer token for `/metrics` |
| `POSTGRES_PASSWORD` | 32 | Database password |
| `REDIS_PASSWORD` | 32 | Cache password |
| `GRAFANA_ADMIN_PASSWORD` | 32 | Grafana login |

Values are base62 (`A–Za–z0–9`) so they can be embedded in `DATABASE_URL` and
`REDIS_URL` without escaping — a generated `@`, `:` or `/` would silently
corrupt a connection string. `REDIS_URL` in `.env` duplicates the password
inline, so the script re-derives it whenever `REDIS_PASSWORD` changes.

**Not generated** — these need a human, and the script says so on exit:
`AWS_*` credentials (prefer an IAM role), `DOMAIN`, `CORS_ORIGIN`,
`WEBAUTHN_RP_ID`, `WEBAUTHN_ORIGIN`, `TRUSTED_PROXIES`.

#### After rotating

`--rotate` writes a timestamped `.env.bak.*` (gitignored, mode 600 — it holds
the *old* secrets, so delete it once you are done). Postgres, Redis and Grafana
keep the credentials they were created with, so recreate the containers:

```bash
docker compose down && docker compose up -d
```

Rotating `JWT_SECRET` invalidates every outstanding admin session.

### Development vs Production

#### Development Mode (Current)
- `NODE_ENV=development`
- Single backend replica (override)
- Localhost HTTP (no SSL)
- Direct Postgres connection (`postgres` service in `docker-compose.yml`)

#### Production Mode — full CLI walkthrough

`docker-compose.prod.yml` (resource limits, per-service replicas) is a
**separate config** from the dev compose above — not a section to uncomment
in the same file. Its Postgres/backup and Caddy/fail2ban services live in
their own files on purpose (`docker-compose.postgres.yml`,
`docker-compose.caddy.yml`), so routine app changes never have the DB or
reverse-proxy config in view. These run purely off pre-built ECR images (no
`build:` blocks) — `--build` does nothing for this stack, unlike dev.

**1. One-time AWS setup** — an S3 bucket for photos + Postgres backups.
Skip bucket creation if one already exists; the CORS/public-read/lifecycle
steps still need to be applied to it once (see **Backups → One-time setup**
above for exactly what each does and why — this just orders them):

```bash
# Only if the bucket doesn't already exist. ap-south-1 (and every region
# except us-east-1) requires an explicit LocationConstraint or this 400s.
aws s3api create-bucket --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" \
  --create-bucket-configuration LocationConstraint="$AWS_REGION"

# Required for the student app's direct-to-S3 photo upload (CORS preflight)
aws s3api put-bucket-cors --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" \
  --cors-configuration '{
    "CORSRules": [{
      "AllowedOrigins": ["https://your-domain.com"],
      "AllowedMethods": ["PUT"],
      "AllowedHeaders": ["content-type"],
      "MaxAgeSeconds": 3600
    }]
  }'

# Required for the admin/mentor panels to display photos (plain <img src>, no presigning)
aws s3api put-public-access-block --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" \
  --public-access-block-configuration \
  BlockPublicAcls=true,IgnorePublicAcls=true,BlockPublicPolicy=false,RestrictPublicBuckets=false
aws s3api put-bucket-policy --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" --policy '{
  "Version": "2012-10-17",
  "Statement": [{
    "Sid": "PublicReadAttendancePhotos",
    "Effect": "Allow",
    "Principal": "*",
    "Action": "s3:GetObject",
    "Resource": "arn:aws:s3:::'"$AWS_S3_BUCKET"'/attendance-photos/*"
  }]
}'

# Optional but recommended — expire old Postgres backup dumps automatically
aws s3api put-bucket-lifecycle-configuration --bucket "$AWS_S3_BUCKET" --region "$AWS_REGION" \
  --lifecycle-configuration '{
    "Rules": [{
      "ID": "expire-postgres-backups",
      "Filter": { "Prefix": "postgres-backups/" },
      "Status": "Enabled",
      "Expiration": { "Days": 90 }
    }]
  }'
```

**2. Clone the repo and configure `.env`** on the server:

```bash
git clone <repository-url> && cd Attendence-GEOTAG-System
cp .env.example .env
```

Edit `.env` and fill in, at minimum: `DOMAIN`, `CORS_ORIGIN` (exact origin,
comma-separated if more than one — `*` is rejected), `WEBAUTHN_RP_ID` (bare
domain, no scheme), `WEBAUTHN_ORIGIN` (`https://` + domain), `PUBLIC_BASE_URL`
(same as `WEBAUTHN_ORIGIN` — used to build `/s/:code` short links),
`AWS_S3_BUCKET`, `AWS_REGION`, `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY`
(prefer an IAM role instead if the host supports it), and `TRUSTED_PROXIES`
(the `web-net` subnet Caddy's container sits on — check with `docker network
inspect <project>_web-net` after first bringing the stack up once, since it's
assigned by Docker).

Then generate everything else (JWT/admin/metrics/DB/Redis/Grafana secrets):

```bash
./scripts/generate-secrets.sh
./scripts/generate-secrets.sh --check   # must exit 0 before continuing
```

**3. Point `Caddyfile.prod` at this domain.** Unlike `.env`, this file is
**not templated** — it's bind-mounted as-is, so the domain and CSP host are
hardcoded and must be hand-edited for a new deployment:
- The site block's domain (`attendixv2.talenciaglobal.com {`) → your `DOMAIN`.
- The `email` directive at the top of the file → a real address for ACME/Let's Encrypt.
- The `Content-Security-Policy` header's `connect-src` **and** `img-src` —
  both hardcode the S3 virtual-hosted host
  (`https://<bucket>.s3.<region>.amazonaws.com`): `connect-src` for the
  student app's direct-upload `PUT`, `img-src` for admin/mentor photo
  display. Update both if `AWS_S3_BUCKET`/`AWS_REGION` differ from what's
  already in the file — a stale `connect-src` breaks photo upload, a stale
  `img-src` breaks photo display, and each fails silently as a CSP violation
  in the browser console, not a server-side error.

**4. Create the external network Caddy expects.** `docker-compose.caddy.yml`
declares `monitoring-net` as `external: true` (so Prometheus can scrape
Caddy's metrics listener without joining `web-net`) — Compose will refuse to
start the stack if this network doesn't already exist, even if you never run
the monitoring stack:

```bash
docker network create monitoring-net
```

**5. Log in to ECR and bring the stack up:**

```bash
aws ecr get-login-password --region "${AWS_REGION:-ap-south-1}" | \
  docker login --username AWS --password-stdin "${ECR_REGISTRY:-559444199242.dkr.ecr.ap-south-1.amazonaws.com}"

docker compose -f docker-compose.prod.yml -f docker-compose.postgres.yml -f docker-compose.caddy.yml pull
docker compose -f docker-compose.prod.yml -f docker-compose.postgres.yml -f docker-compose.caddy.yml up -d
```

Add `-f docker-compose.monitoring.yml` to both commands above if you also
want Prometheus/Loki/Grafana — it redeclares `web-net` itself, so it doesn't
need its own external-network step.

**6. Verify:**

```bash
curl https://your-domain.com/health
docker compose -f docker-compose.prod.yml -f docker-compose.postgres.yml -f docker-compose.caddy.yml ps
docker compose -f docker-compose.prod.yml -f docker-compose.postgres.yml -f docker-compose.caddy.yml logs -f backend caddy
```

**7. Create the first admin account.** This route (`backend-rust/src/controllers/admin/auth.rs`)
only ever bootstraps the *first* `super_admin` — every account after that is
created through the authenticated User Management panel, not this endpoint:

```bash
curl -X POST https://your-domain.com/api/admin/register \
  -H 'Content-Type: application/json' \
  -d '{
    "username": "owner",
    "email": "owner@your-domain.com",
    "password": "a-strong-password",
    "adminSecret": "<ADMIN_SECRET from .env>"
  }'
```

---

## 🧪 Testing

### Check System Health

```bash
# Backend health
curl http://localhost/health

# Postgres readiness
docker exec attendance-postgres pg_isready -U postgres

# Redis connection
docker exec attendance-redis redis-cli ping

# Check backend replicas
docker-compose ps backend
```

### Load Testing

```bash
# Install artillery
npm install -g artillery

# Run test
artillery quick --count 100 --num 10 http://localhost/health
```

---

## 📈 Monitoring

### View Logs

```bash
# All services
docker-compose logs -f

# Specific service
docker-compose logs -f backend
docker-compose logs -f redis
docker-compose logs -f caddy

# Postgres logs
docker logs attendance-postgres

# Backup job logs
docker logs attendance-pg-backup
```

### Check Resource Usage

```bash
# Container stats
docker stats
```

---

## 🔍 Troubleshooting

### Postgres Not Starting / Migrations Failing

```bash
# Check Postgres logs
docker-compose logs postgres

# Check backend logs (sqlx migrations run at backend startup)
docker-compose logs backend

# Connect directly to inspect state
docker exec -it attendance-postgres psql -U postgres -d attendance_geotag
```

### Redis Connection Issues

```bash
# Test connection
docker exec attendance-redis redis-cli ping

# Check logs
docker-compose logs redis
```

### Backend Not Starting

```bash
# Check backend logs
docker-compose logs backend

# Check Postgres connection
docker exec attendance-backend-1 pg_isready -h postgres -U postgres

# Check Redis connection
docker exec attendance-backend-1 curl redis://redis:6379
```

### Backup Job Not Running

```bash
# Confirm the container is up and the crontab rendered correctly
docker exec attendance-pg-backup cat /etc/crontabs/root

# Trigger a manual run to see the actual error
docker exec attendance-pg-backup /usr/local/bin/backup.sh
```

---

## 🛠️ Maintenance

### Stop All Services

```bash
docker-compose down
```

### Stop and Remove Volumes (Reset)

```bash
docker-compose down -v
```

### Restart Specific Service

```bash
docker-compose restart backend
docker-compose restart redis
```

### Scale Backend (Manual)

```bash
docker-compose up -d --scale backend=5
```

---

## 📝 Environment Variables Reference

| Variable | Default | Description |
|----------|---------|-------------|
| `NODE_ENV` | development | Environment mode |
| `DOMAIN` | localhost | Domain for SSL |
| `STORAGE_PROVIDER` | s3 | Storage backend |
| `AWS_S3_BUCKET` | - | S3 bucket name (photos + Postgres backups) |
| `AWS_REGION` | us-east-1 | S3 bucket region — must match the bucket's actual region and the CORS/CSP host below |
| `AWS_ACCESS_KEY_ID` | - | AWS access key |
| `AWS_SECRET_ACCESS_KEY` | - | AWS secret key |
| `POSTGRES_DB` | attendance_geotag | Postgres database name |
| `POSTGRES_USER` | postgres | Postgres user |
| `POSTGRES_PASSWORD` | - | Postgres password |
| `PG_MAX_POOL_SIZE` | 300 | Connection pool max |
| `PG_MIN_POOL_SIZE` | 20 | Connection pool min |
| `BACKUP_CRON_SCHEDULE` | `0 2 * * *` | pg-backup cron schedule |
| `BACKUP_RETENTION_DAYS` | 30 | Local dump retention inside the pg-backup container |
| `REDIS_URL` | redis://redis:6379 | Redis connection |

---

## 🎯 Performance Tuning

### Postgres

- Connection pool: 300 (already optimized)
- Indexed queries on `session_id` + `roll_number`, plus the other indexes in
  `backend-rust/migrations/0001_initial_schema.sql`

### Redis

- Cache TTL: 300 seconds (5 minutes)
- Memory limit: 512MB
- Eviction: LRU policy

### Backend

- Multiple container replicas behind Caddy
- Max memory: 2GB per container (see `docker-compose.prod.yml`)
- Auto-restart on crash

---

## 📞 Support

For issues or questions:
1. Check logs: `docker-compose logs -f`
2. Check health: `curl http://localhost/health`
3. Review this guide

---

## ✅ Next Steps

1. **Test the system**: `docker-compose up -d`
2. **Create admin account**: Use `/api/admin/register`
3. **Create location**: Admin panel → Locations
4. **Create session**: Admin panel → Sessions
5. **Share attendance link**: `/attend/<token>`

The system is production-ready! 🎉
