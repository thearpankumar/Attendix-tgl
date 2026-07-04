# 🚀 Deployment Guide

## Architecture Overview

This system is designed to handle **1000+ concurrent users** with the following architecture:

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
│ PM2  │ │ PM2  │ │ PM2  │
└──┬───┘ └──┬───┘ └──┬───┘
   │        │        │
   └────────┴────────┘
        │        │
  ┌─────┴────┐ ┌─┴────┐
  │ MongoDB  │ │Redis │
  │ Replica  │ │Cache │
  │ Set (3)  │ │      │
  └──────────┘ └──────┘
```

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

### Setup Script Capabilities

| Feature | Description |
|---------|-------------|
| **Cross-platform** | Works on Linux (Ubuntu/Debian) and macOS (Intel & Apple Silicon) |
| **Docker Installation** | Installs Docker Engine (Linux) or Docker Desktop (macOS) |
| **Node.js Installation** | Installs Node.js 22 LTS via NVM |
| **Version Detection** | Detects existing installations and warns without overwriting |
| **Dependency Installation** | Installs npm packages for backend and frontend |
| **Environment Setup** | Copies .env.example to .env |
| **Docker Management** | Start, stop, logs, status, reset operations |
| **Development Servers** | Run backend (nodemon) and frontend (Vite) dev servers |
| **Testing** | Run backend tests and linter |

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
- ✅ MongoDB settings
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
| **Admin Panel** | http://localhost/admin |
| **Student Page** | http://localhost/attend/\<token\> |
| **API Health** | http://localhost/health |
| **Direct Backend** | http://localhost:5000 |

---

## 📊 Services Breakdown

### Backend (3 replicas with PM2)
- **Image**: Node.js 18 Alpine
- **Process Manager**: PM2 (2 workers per container)
- **Total Workers**: 6 Node.js processes
- **Port**: 5000 (internal)
- **Health Check**: `/health` endpoint

### MongoDB Replica Set
- **3 Nodes**:
  - `mongo1`: Primary candidate (port 27017)
  - `mongo2`: Secondary (port 27018)
  - `mongo3`: Arbiter (port 27019)
- **Automatic failover**: ✅
- **Data redundancy**: 2 copies

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

---

## 🔧 Configuration

### Development vs Production

#### Development Mode (Current)
- `NODE_ENV=development`
- Single backend replica (override)
- Localhost HTTP (no SSL)
- Direct MongoDB connection

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

---

## 🧪 Testing

### Check System Health

```bash
# Backend health
curl http://localhost/health

# MongoDB replica set status
docker exec attendance-mongo1 mongosh --eval "rs.status().members.forEach((m,i) => print(i+1, m.name, m.stateStr))"

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

# MongoDB logs
docker logs attendance-mongo1
```

### Check Resource Usage

```bash
# Container stats
docker stats

# Backend logs (PM2)
docker exec attendance-backend-1 pm2 logs
```

---

## 🔍 Troubleshooting

### MongoDB Replica Set Not Initializing

```bash
# Check if init script ran
docker-compose logs mongodb-init

# Manually initialize
docker exec attendance-mongo1 mongosh --file /init-mongo.js

# Check status
docker exec attendance-mongo1 mongosh --eval "rs.status()"
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

# Check MongoDB connection
docker exec attendance-backend-1 curl mongodb://mongo1:27017

# Check Redis connection
docker exec attendance-backend-1 curl redis://redis:6379
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
| `AWS_S3_BUCKET` | - | S3 bucket name |
| `AWS_ACCESS_KEY_ID` | - | AWS access key |
| `AWS_SECRET_ACCESS_KEY` | - | AWS secret key |
| `MONGODB_POOL_MAX` | 300 | Connection pool max |
| `MONGODB_POOL_MIN` | 20 | Connection pool min |
| `REDIS_URL` | redis://redis:6379 | Redis connection |

---

## 🎯 Performance Tuning

### MongoDB

- Connection pool: 300 (already optimized)
- Indexed queries on `sessionId` + `rollNumber`
- Replica set read preferences configurable

### Redis

- Cache TTL: 300 seconds (5 minutes)
- Memory limit: 512MB
- Eviction: LRU policy

### Backend

- PM2 cluster: 2 workers per container
- Max memory: 1.5GB per container
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
