# Geotag-Based Attendance System

[![Node.js](https://img.shields.io/badge/Node.js-22%2B-green?logo=node.js&logoColor=white)](https://nodejs.org/)
[![MongoDB](https://img.shields.io/badge/MongoDB-7.0-47A248?logo=mongodb&logoColor=white)](https://www.mongodb.com/)
[![React](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=black)](https://reactjs.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0-3178C6?logo=typescript&logoColor=white)](https://www.typescriptlang.org/)
[![Docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker&logoColor=white)](https://www.docker.com/)
[![Prometheus](https://img.shields.io/badge/Prometheus-Monitoring-E6522C?logo=prometheus&logoColor=white)](https://prometheus.io/)
[![Grafana](https://img.shields.io/badge/Grafana-Dashboards-F46800?logo=grafana&logoColor=white)](https://grafana.com/)
[![License](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

[![Tests Backend](https://img.shields.io/badge/Tests-328%20passing-brightgreen)](backend/tests/)
[![Coverage](https://img.shields.io/badge/Coverage-Comprehensive-blue)](backend/tests/)
[![WebAuthn](https://img.shields.io/badge/WebAuthn-Enabled-purple)](https://webauthn.io/)
[![MediaPipe](https://img.shields.io/badge/MediaPipe-Face_Detection-00B2A9?logo=google&logoColor=white)](https://developers.google.com/mediapipe)

A production-ready attendance system integrating high-accuracy GPS geolocation, facial recognition, WebAuthn biometric authentication, and comprehensive device integrity verification. Built to scale for high concurrency environments using Node.js, React, and MongoDB.

---

## System Overview

The Geotag-Based Attendance System is designed for academic and corporate environments requiring indisputable proof of presence. It ensures that users are physically present at a designated location using a combination of strict GPS boundaries (geofencing), biometric hardware verification (WebAuthn), real-time facial detection (MediaPipe), and device anti-tampering heuristics.

The system employs an "Observer Pattern" for security enforcement: anomalies (such as emulator usage or GPS spoofing) do not outright block the user from submitting attendance, but instead securely flag the record for administrative review. This guarantees a frictionless user experience while providing administrators with total auditing visibility.

---

## Table of Contents

- [System Overview](#system-overview)
- [Core Features](#core-features)
- [Architecture & Data Flow](#architecture--data-flow)
- [Device Security & Integrity](#device-security--integrity)
- [Workflows](#workflows)
- [Installation & Deployment](#installation--deployment)
- [Observability](#observability)
- [API Documentation](#api-documentation)
- [Development Guide](#development-guide)
- [License](#license)

---
## Core Features

- **WebAuthn Biometrics**: Hardware-level authentication (Face ID, Touch ID, Windows Hello) to prevent credential sharing and impersonation.
- **Geofence Validation**: High-accuracy coordinate checking against dynamic administrative perimeters.
- **Facial Detection**: Real-time bounding-box face detection via MediaPipe during photo capture.
- **Device Integrity Pipeline**: Client and server-side heuristics to detect Android/iOS emulators, mock locations, and CPU timing manipulations.
- **Administrative Security Review**: Advanced dashboard for bulk verification, session filtering, and anomaly investigation.
- **Centralized Observability**: Integrated Prometheus, Grafana, and Loki stack for system telemetry and log aggregation.
- **Enterprise Storage**: S3-compatible cloud storage for secure, presigned, direct-to-bucket photo uploads.

---

## Architecture & Data Flow

### System Architecture

```mermaid
graph TB
    Internet[Internet Traffic]
    
    subgraph "Reverse Proxy Layer"
        Caddy[Caddy Load Balancer / SSL Termination]
    end
    
    subgraph "Application Layer"
        Backend1[Node.js API Node 1]
        Backend2[Node.js API Node 2]
    end
    
    subgraph "Frontend Layer"
        AdminFE[React Admin SPA]
        StudentFE[Student PWA]
    end
    
    subgraph "Database & Cache Layer"
        Mongo[MongoDB Replica Set]
        Redis[Redis Session Cache]
    end
    
    subgraph "Security & ML Layer"
        MediaPipe[MediaPipe Face Detection]
        DeviceHeuristics[Device Integrity Middlewares]
    end
    
    subgraph "Observability Layer"
        Prometheus[Prometheus Metrics]
        Grafana[Grafana Dashboards]
        Loki[Loki Log Aggregation]
    end
    
    Internet --> Caddy
    Caddy --> AdminFE
    Caddy --> StudentFE
    Caddy --> Backend1
    Caddy --> Backend2
    
    Backend1 --> Redis
    Backend2 --> Redis
    
    Backend1 --> Mongo
    Backend2 --> Mongo
    
    Backend1 --> DeviceHeuristics
    Backend2 --> DeviceHeuristics
    
    StudentFE --> MediaPipe
    
    Backend1 -.-> Prometheus
    Backend1 -.-> Loki
```

### Security Middleware Data Flow

```mermaid
sequenceDiagram
    participant Client as Student Device
    participant Gateway as API Router
    participant Emulator as Emulator Detection
    participant GPS as GPS Validation
    participant Integrity as Integrity Middleware
    participant DB as MongoDB

    Client->>Gateway: POST /attendance (Payload, Telemetry)
    Gateway->>Emulator: Check WebGL/Canvas Profiles
    Emulator-->>Gateway: Append Emulator Flags
    Gateway->>GPS: Analyze Coordinate History & Speed
    GPS-->>Gateway: Append GPS Flags
    Gateway->>Integrity: Verify CPU/Pointer Telemetry
    Integrity-->>Gateway: Append Integrity Flags
    Gateway->>DB: Save Record (Flagged if anomalies exist)
    DB-->>Client: 201 Created (Success)
```

### Data Flow Architecture

```mermaid
graph LR
    subgraph "Client Layer"
        Browser[Browser]
        Mobile[Mobile PWA]
    end
    
    subgraph "Reverse Proxy"
        Caddy[Caddy SSL/Gzip]
    end
    
    subgraph "API Gateway"
        RateLimit[Rate Limiter]
        Auth[JWT Auth Middleware]
    end
    
    subgraph "Application Services"
        SessionService[Session Service]
        SecurityService[Integrity & GPS Validation]
        AttendanceService[Attendance Service]
        WebAuthnService[WebAuthn Service]
        StorageService[Storage Service]
    end
    
    subgraph "Cache Layer"
        Redis[Redis Cache]
    end
    
    subgraph "Data Layer"
        MongoDB[(MongoDB Replica Set)]
    end
    
    subgraph "Storage Layer"
        S3[(AWS S3 / Cloudinary)]
    end
    
    Browser -->|HTTPS| Caddy
    Mobile -->|HTTPS| Caddy
    Caddy --> RateLimit
    RateLimit --> Auth
    
    Auth --> SessionService
    Auth --> WebAuthnService
    Auth --> StorageService
    Auth --> SecurityService
    SecurityService --> AttendanceService
    
    SessionService -->|Cache-Aside| Redis
    AttendanceService -->|Cache-Aside| Redis
    WebAuthnService -->|Cache-Aside| Redis
    
    SessionService --> MongoDB
    AttendanceService --> MongoDB
    WebAuthnService --> MongoDB
    
    StorageService -->|Presigned URLs| S3
```

---

## Device Security & Integrity

The system runs a multi-layered security pipeline to guarantee submission authenticity.

### Client-Side Heuristics
- **Pointer Event Verification**: Detects automated scripts by validating touch points versus user-agent declarations.
- **Timing Manipulation**: Detects clock tampering by benchmarking standard computational loops against `performance.now()`.
- **MediaPipe Verification**: Client-side ML models verify human facial presence before upload presigned URLs are generated.

### Server-Side Middlewares
- **Emulator Detection**: Analyzes WebGL renderer strings, canvas capabilities, and device memory signatures to flag Android/iOS emulators.
- **GPS History Service**: Tracks sequential submissions to calculate travel speed. Submissions requiring impossible velocities (e.g., traveling 5km in 2 seconds) are flagged.
- **Mock Location Detection**: Flags perfectly uniform coordinates or missing altitude/heading data typical of GPS spoofing applications.

---

## Workflows

### Administrative Workflow
1. **Infrastructure Setup**: Define physical locations via coordinates and radius.
2. **Session Generation**: Initialize time-bound attendance sessions linked to a location.
3. **Distribution**: Share securely generated short links or QR codes.
4. **Monitoring**: Track real-time attendance influx via the session dashboard.
5. **Security Review**: Investigate the Security Review panel for flagged anomalies. Filter sessions by date and location.
6. **Verification**: Execute single or bulk verifications for legitimate records.

### Student Workflow
1. **Access**: Open the shared short-link via a mobile device.
2. **Authentication**: Input roll number and complete hardware WebAuthn biometric verification.
3. **Capture**: Grant camera permissions, center face for MediaPipe detection, and capture live photo.
4. **Geolocation**: Grant location permissions to establish geofence compliance.
5. **Submission**: Submit payload. Background integrity checks execute silently.

---

## Installation & Deployment

### Prerequisites
- Docker and Docker Compose
- Node.js 22.x (For local development without containers)
- AWS S3 compatible storage (or Cloudinary)

### Automated Setup (Recommended)

Use the provided setup script for a guided installation:

```bash
# Make the script executable
chmod +x setup.sh

# Run the interactive menu
./setup.sh menu

# Or use command-line options
./setup.sh check     # Check system requirements
./setup.sh install   # Install Docker, Node.js, and dependencies
./setup.sh up        # Start Docker Compose
./setup.sh help      # Show all available commands
```

**Setup Script Features:**
- Cross-platform support (Linux Ubuntu/Debian & macOS Intel/Apple Silicon)
- Installs Docker & Docker Compose (if not present)
- Installs Node.js 22 LTS via NVM (if not present)
- Detects existing versions and warns without overwriting
- Installs project dependencies
- Creates .env file from .env.example
- Interactive menu and CLI options

### Quick Start via Docker

1. Clone the repository and configure the environment:
```bash
git clone <repository-url>
cd Attendence-GEOTAG-System
cp .env.example .env
```

2. Configure primary environment variables within `.env`:
```env
# Infrastructure
NODE_ENV=production
PORT=5000
MONGODB_URI=mongodb://mongo:27017/attendance
REDIS_URL=redis://redis:6379

# Secrets
JWT_SECRET=your_secure_jwt_secret
ADMIN_SECRET=your_secure_admin_secret

# Storage (S3 Configuration)
STORAGE_PROVIDER=s3
AWS_S3_BUCKET=your-bucket-name
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key

# WebAuthn
WEBAUTHN_RP_NAME=Corporate Identity Provider
WEBAUTHN_RP_ID=your-domain.com
WEBAUTHN_ORIGIN=https://your-domain.com
```

3. Deploy the stack:
```bash
docker-compose up -d --build
```
This commands provisions the Backend API, Admin Frontend, Student PWA, MongoDB Replica Set, Redis cache, and the entire Prometheus/Grafana observability stack.

4. Create the Super-Admin Account (one-time bootstrap):

`POST /api/admin/register` is a **one-time bootstrap endpoint**, not a general registration route. It is reached through Caddy on port `80`/`443` (the backend's own port is never published to the host — see `docker-compose.yml`), gated by the shared `ADMIN_SECRET` from `.env`. The **first** successful call creates the account as `role: "super_admin"`; every call after that returns `404 Not Found` regardless of how correct `adminSecret` is, because account creation moves exclusively to the authenticated **User Management** page in the admin panel (`/owner-of-attendix-xyz/users`) from then on. Pick a real username/password here — there is no way to run this endpoint again to create a second one.

```bash
curl -X POST http://localhost/api/admin/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "your-username",
    "email": "your-email@example.com",
    "password": "your-strong-password",
    "adminSecret": "your-admin-secret-from-.env"
  }'
```

Log in with those credentials at `/owner-of-attendix-xyz/login`, then use **User Management** to create every admin/mentor account after that (username, college email, full name, college name, password, role — `admin` for a mentor who marks attendance, `super_admin` for another full-access account).

---

## Observability

The project includes an embedded telemetry stack for production monitoring.

- **Prometheus**: Scrapes application metrics (CPU usage, memory heap, request latency, concurrent active sessions) on port `:9090`.
- **Grafana**: Pre-configured dashboards visualizing API health, anomaly detection rates, and system integrity scores, accessible on port `:3001`.
- **Loki & Promtail**: Aggregates application logs, streamlining debugging and security audits. Configured to persist logs to the unified S3 bucket.

---

## API Documentation

The REST API utilizes standard HTTP verbs and JSON payloads. Admin routes require a Bearer JWT.

### Administrative Endpoints
- `POST /api/admin/login` - Issue JWT for administrative access.
- `POST /api/admin/locations` - Provision a new geofenced location.
- `POST /api/admin/sessions` - Initialize an attendance session.
- `GET /api/admin/sessions/:id/security-flags` - Retrieve aggregated anomaly metrics for a session.
- `PUT /api/admin/attendance/verify/bulk` - Execute bulk verification state changes.

### Student Endpoints
- `POST /s/:shortCode/webauthn/register/start` - Initiate hardware biometric registration.
- `POST /s/:shortCode/webauthn/auth/finish` - Conclude biometric verification.
- `POST /api/attend/:token/upload-url` - Request an S3 presigned URL for direct image upload.
- `POST /api/attend/:token` - Finalize attendance submission containing GPS coordinates and photo references.

---

## Development Guide

### Local Development Setup

To run the application services directly on the host machine:

#### Backend
```bash
cd backend
npm install
npm run dev
```

#### Admin Frontend
```bash
cd frontend/admin
npm install
npm run dev
```

#### Testing
The system maintains a comprehensive suite of unit and integration tests using Jest and Vitest.
```bash
# Run backend tests
cd backend && npm test

# Run student frontend integration tests
cd frontend/student && npm test
```

---

## License

This project is licensed under the MIT License. Please review the `LICENSE` file in the repository root for details.
