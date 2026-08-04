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

### Development vs Production

#### Development Mode (Current)
- `NODE_ENV=development`
- Single backend replica (override)
- Localhost HTTP (no SSL)
- Direct Postgres connection (`postgres` service in `docker-compose.yml`)

#### Production Mode
1. Update `.env`:
   ```bash
   NODE_ENV=production
   DOMAIN=your-domain.com
   ```

2. Uncomment production section in `Caddyfile`

3. Restart:
   ```bash
   docker-compose down
   docker-compose up -d --build
   ```

> **Note:** `docker-compose.prod.yml` (the fuller prod stack with resource
> limits and per-service replicas) is a separate config from the dev compose
> above. Its Postgres/backup and Caddy/fail2ban services live in their own
> files on purpose — `docker-compose.postgres.yml` and
> `docker-compose.caddy.yml` — so routine app changes never have the DB or
> reverse-proxy config in view. Run all three together:
> ```bash
> aws ecr get-login-password --region ${AWS_REGION:-ap-south-1} | docker login --username AWS --password-stdin ${ECR_REGISTRY}
> docker compose -f docker-compose.prod.yml -f docker-compose.postgres.yml -f docker-compose.caddy.yml up -d --build
> ```

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
