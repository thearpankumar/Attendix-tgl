# Hardware Requirements & Scalability Guide

## Executive Summary

**System Capacity:** The attendance system can handle **1,000+ concurrent students** with proper configuration.

---

## Minimum Hardware Requirements (1,000 Students)

| Component | Specification | Notes |
|-----------|---------------|-------|
| **CPU** | 2 vCPUs (t3.medium) | Sufficient for peak ~50 RPS |
| **RAM** | 4GB | Node heap 1.5GB + OS + MongoDB |
| **Storage** | 20GB SSD | OS + logs + temp uploads |
| **Network** | 500 Mbps | Photo uploads bandwidth |
| **MongoDB** | Atlas M10 or 2GB RAM self-hosted | Connection pool: 300 |

---

## Recommended Hardware (Production)

| Component | Specification | Capacity |
|-----------|---------------|----------|
| **CPU** | 4 vCPUs (c5.large) | 2,000-5,000 students |
| **RAM** | 8GB | Handles burst traffic |
| **Storage** | 50GB SSD | Extended logs + backups |
| **Network** | 1 Gbps | High photo upload volume |

---

## Performance Estimates

### Submission Window Analysis

| Students | Window | Avg RPS | Peak RPS | Server Load |
|----------|--------|---------|-----------|-------------|
| 100 | 30 min | 0.06 | 5 | Minimal |
| 500 | 30 min | 0.28 | 25 | Low |
| 1,000 | 30 min | 0.56 | 50 | Moderate |
| 2,000 | 30 min | 1.1 | 100 | High |
| 5,000 | 30 min | 2.8 | 250 | Very High |

### Photo Upload Impact

| Storage Type | Latency | Server Bandwidth | Students/Min |
|--------------|--------|------------------|--------------|
| **Cloudinary** | 500-1500ms | 100MB overhead | ~40 |
| **S3 Direct** | 200-400ms | 0MB (direct) | ~150 |

---

## Connection Pooling

### MongoDB Settings

```javascript
// Current configuration (db.js)
maxPoolSize: 300      // Handle burst of 1000 students
minPoolSize: 20       // Keep warm connections
```

### Per-Request DB Queries

Each attendance submission performs:
1. Session lookup (token hash)
2. Location populate
3. Duplicate check (indexed)
4. Attendance insert

**Total queries per submission:** 4
**With pool of 300:** Can handle 75 concurrent submissions (4 queries each)

---

## Memory Analysis

### Node.js Memory Usage

| Component | Size | Notes |
|-----------|------|-------|
| Base Node | ~50MB | Runtime overhead |
| Express | ~20MB | Framework |
| MongoDB driver | ~30MB | Connection pool |
| Per-request base64 | ~5MB | Photo in memory |
| **Peak (100 concurrent)** | ~500MB | Manageable |

**Recommended heap:** `NODE_OPTIONS=--max-old-space-size=2048`

---

## Bandwidth Requirements

### Photo Size Estimates

| Quality | Size | Reason |
|---------|------|--------|
| High (0.8) | ~200KB | Default |
| Medium (0.7) | ~100KB | Recommended |
| Low (0.5) | ~50KB | Low bandwidth mode |

### Monthly Bandwidth (1,000 students/day)

| Traffic Type | Monthly | Cost Factor |
|--------------|---------|--------------|
| Uploads | 3GB | Students → Cloudinary/S3 |
| Downloads | 30GB | Admin viewing photos |
| API calls | 1GB | CRUD operations |
| **Total** | ~35GB | |

---

## Cloud Instance Recommendations

### AWS EC2 Options

| Instance | vCPU | RAM | Cost/Mo | Capacity |
|----------|------|-----|---------|----------|
| t3.medium | 2 | 4GB | $30 | 1,000 students |
| c5.large | 2 | 4GB | $60 | 2,000 students |
| c5.xlarge | 4 | 8GB | $120 | 5,000 students |

### MongoDB Atlas Options

| Tier | RAM | Storage | Cost/Mo | Capacity |
|------|-----|---------|---------|----------|
| M10 | 2GB | 10GB | $57 | 10,000 records/day |
| M20 | 4GB | 20GB | $150 | 50,000 records/day |
| M30 | 8GB | 40GB | $540 | High volume |

---

## Monthly Cost Estimates

### Option A: Cloudinary (Current Setup)

| Service | Cost/Month |
|---------|------------|
| EC2 (t3.medium) | $30 |
| Cloudinary Free | $0 (25GB) |
| MongoDB Atlas M10 | $57 |
| Ngrok (free) | $0 |
| **TOTAL** | **$87/month** |

### Option B: AWS S3 Direct

| Service | Cost/Month |
|---------|------------|
| EC2 (t3.medium) | $30 |
| S3 Storage (3GB) | $0.07 |
| S3 Requests | $3 |
| S3 Bandwidth | $10 |
| MongoDB Atlas M10 | $57 |
| **TOTAL** | **$100/month** |

---

## Scaling Optimizations

### Current Optimizations (Applied)

1. ✅ MongoDB connection pool: 300
2. ✅ Rate limiting with standard headers
3. ✅ Database indexes on sessionId + rollNumber
4. ✅ Connection monitoring

### Recommended Future Optimizations

1. **Redis caching** for session validation
2. **Queue system** (Bull/Agenda) for photo uploads
3. **Read replicas** for MongoDB (heavy read traffic)
4. **CDN** for static assets (CloudFront)
5. **Load balancer** for horizontal scaling

---

## Monitoring Recommendations

### Key Metrics to Track

| Metric | Threshold | Action |
|--------|-----------|--------|
| CPU Usage | > 80% | Scale up |
| Memory Usage | > 85% | Scale up |
| MongoDB Connections | > 250 | Increase pool |
| Request Latency | > 1s | Investigate |
| Error Rate | > 1% | Alert |

### Recommended Tools

- **CloudWatch** (AWS) - Infrastructure metrics
- **PM2** - Node.js process management
- **MongoDB Atlas Monitoring** - Database metrics
- **Ngrok Dashboard** - Tunnel monitoring

---

## Quick Reference Commands

### Check Node.js memory
```bash
node --expose-gc -e "console.log(process.memoryUsage())"
```

### Monitor MongoDB connections
```javascript
db.serverStatus().connections
```

### Load test with 1000 users
```bash
npx artillery quick --count 1000 --num 1 http://localhost:5000/health
```

---

## Troubleshooting High Load

### Symptoms & Solutions

| Symptom | Likely Cause | Solution |
|---------|--------------|----------|
| Slow uploads | Network bottleneck | Increase bandwidth |
| MongoDB timeouts | Pool exhausted | Increase maxPoolSize |
| Memory errors | Large photos | Reduce quality to 0.6 |
| High CPU | Too many requests | Scale horizontally |
| Connection drops | MongoDB limits | Create read replica |

---

## Conclusion

The system is **production-ready** for 1,000 concurrent students with:
- t3.medium EC2 instance (2 vCPU, 4GB RAM)
- MongoDB Atlas M10 cluster
- Cloudinary free tier or S3
- Total cost: ~$87-100/month

For larger deployments, scale to c5.large and M20 MongoDB.
