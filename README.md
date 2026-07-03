# Geotag-Based Attendance System

[![Node.js](https://img.shields.io/badge/Node.js-18%2B-green?logo=node.js&logoColor=white)](https://nodejs.org/)
[![MongoDB](https://img.shields.io/badge/MongoDB-7.0-47A248?logo=mongodb&logoColor=white)](https://www.mongodb.com/)
[![React](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=black)](https://reactjs.org/)
[![Docker](https://img.shields.io/badge/Docker-Ready-2496ED?logo=docker&logoColor=white)](https://www.docker.com/)
[![License](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

[![Tests Backend](https://img.shields.io/badge/Tests-269%20passing-brightgreen)](backend/tests/)
[![Coverage](https://img.shields.io/badge/Coverage-Comprehensive-blue)](backend/tests/)
[![WebAuthn](https://img.shields.io/badge/WebAuthn-Enabled-purple)](https://webauthn.io/)

A production-ready attendance system with GPS geolocation, camera verification, and **WebAuthn biometric authentication** for secure student identity verification.

---

## Table of Contents

- [Features](#features)
- [Tech Stack](#tech-stack)
- [Storage Options](#storage-options)
- [Quick Start](#quick-start)
- [WebAuthn Biometric Verification](#webauthn-biometric-verification)
- [Usage](#usage)
- [API Endpoints](#api-endpoints)
- [Development](#development)
- [Architecture](#architecture)
- [Environment Variables](#environment-variables-reference)
- [Security](#security-features)
- [Testing](#testing)
- [Scaling](#scaling)
- [Hardware Requirements](#hardware-requirements)
- [License](#license)

---

## Features

| Feature | Description |
|---------|-------------|
| **WebAuthn Biometric** | Face ID/Touch ID verification prevents impersonation |
| **Geotag Verification** | Students must be within specified radius of location |
| **Camera Capture** | Photo verification for each attendance submission |
| **TOTP Security** | Time-based codes for session authentication |
| **Device Fingerprinting** | Detects multi-device and suspicious activity |
| **Short Links** | Easy-to-share session URLs with rotation support |
| **Rotating Tokens** | Admin can rotate session links to prevent sharing |
| **Duplicate Prevention** | Same roll number cannot submit twice per session |
| **Real-time Stats** | Live attendance count with polling-based updates |
| **Flagged Attendance** | Automatic flagging of suspicious submissions |
| **NoSQL Performance** | MongoDB handles 1000+ concurrent submissions |
| **Flexible Storage** | Choose between Cloudinary or AWS S3 |
| **Direct Upload** | S3 presigned URLs for direct browser uploads |
| **Docker Ready** | One-command deployment with docker-compose |
| **CI/CD Pipeline** | GitHub Actions for automated testing and deployment |

---

## Tech Stack

| Component | Technology |
|-----------|------------|
| **Backend** | Node.js + Express.js |
| **Database** | MongoDB 7.0 |
| **Biometric Auth** | WebAuthn (@simplewebauthn/server) |
| **TOTP** | Custom implementation with SHA-256 |
| **Image Storage** | Cloudinary OR AWS S3 (configurable) |
| **Admin Panel** | React 18 + Vite |
| **Student Page** | Vanilla JS (lightweight, mobile-optimized) |
| **Authentication** | JWT with bcrypt password hashing |
| **Testing** | Jest (269 tests) |
| **Deployment** | Docker Compose |

---

## Storage Options

The system supports two storage backends, configurable via `STORAGE_PROVIDER` environment variable.

### Option 1: Cloudinary (Default)

| Pros | Cons |
|------|------|
| Automatic image optimization | Images pass through backend server |
| Built-in transformations | Slightly higher latency for large files |
| Easy setup | Limited free tier bandwidth |
| Free tier: 25GB storage + bandwidth | |

### Option 2: AWS S3

| Pros | Cons |
|------|------|
| Direct browser-to-S3 upload (presigned URLs) | Requires AWS account setup |
| Lower backend load | Need to configure CORS on bucket |
| Better for high-volume uploads | |
| Pay only for what you use | |

---

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Cloudinary account (free tier available) OR AWS account with S3 access

### 1. Clone and Configure

```bash
git clone <repository-url>
cd Attendence-GEOTAG-System
cp .env.example .env
```

Edit `.env` with your credentials:

**For Cloudinary:**
```bash
STORAGE_PROVIDER=cloudinary
JWT_SECRET=your-secret-key
ADMIN_SECRET=your-admin-secret
CLOUDINARY_CLOUD_NAME=your-cloud-name
CLOUDINARY_API_KEY=your-api-key
CLOUDINARY_API_SECRET=your-api-secret

# WebAuthn Configuration
WEBAUTHN_RP_NAME=Your Institution Name
WEBAUTHN_RP_ID=localhost
WEBAUTHN_ORIGIN=http://localhost:5000
```

**For AWS S3:**
```bash
STORAGE_PROVIDER=s3
JWT_SECRET=your-secret-key
ADMIN_SECRET=your-admin-secret
AWS_S3_BUCKET=your-bucket-name
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=your-access-key
AWS_SECRET_ACCESS_KEY=your-secret-key

# WebAuthn Configuration
WEBAUTHN_RP_NAME=Your Institution Name
WEBAUTHN_RP_ID=your-domain.com
WEBAUTHN_ORIGIN=https://your-domain.com
```

### 2. S3 CORS Configuration (Required for S3 only)

```json
[
  {
    "AllowedOrigins": ["*"],
    "AllowedMethods": ["GET", "PUT", "POST", "HEAD"],
    "AllowedHeaders": ["*"],
    "ExposeHeaders": ["ETag"],
    "MaxAgeSeconds": 3000
  }
]
```

### 3. Run with Docker Compose

```bash
docker-compose up -d
```

This starts:
- MongoDB on port 27017
- Backend API on port 5000
- Admin panel on port 3000
- Student page on port 8080

### 4. Create Admin Account

```bash
curl -X POST http://localhost:5000/api/admin/register \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "email": "admin@example.com",
    "password": "admin123",
    "adminSecret": "your-admin-secret"
  }'
```

### 5. Access the Application

| Interface | URL |
|-----------|-----|
| Admin Panel | http://localhost:3000 |
| Student Page | http://localhost:8080 |
| API Health | http://localhost:5000/health |

---

## WebAuthn Biometric Verification

### Overview

The system supports WebAuthn-based biometric authentication using platform authenticators (Face ID, Touch ID, Windows Hello, Android fingerprint). This adds a strong security layer to prevent impersonation.

### How It Works

```
┌──────────────────────────────────────────────────────────────────┐
│                    Student Enrollment Flow                        │
├──────────────────────────────────────────────────────────────────┤
│  1. Student enters roll number                                   │
│  2. System checks if already enrolled                            │
│  3. If not enrolled → Registration flow                          │
│     - Browser prompts for biometric (Face ID/Touch ID)           │
│     - Credential stored with student ID                          │
│  4. If enrolled → Authentication flow                            │
│     - Browser prompts for biometric verification                  │
│     - Session counter checked for replay attacks                 │
│  5. After biometric success → Complete attendance form           │
└──────────────────────────────────────────────────────────────────┘
```

### Security Features

| Feature | Implementation |
|---------|---------------|
| **Platform Authenticator Only** | No USB keys, only built-in biometrics |
| **User Verification Required** | Must use biometric or device PIN |
| **Sign Counter Tracking** | Detects credential cloning attacks |
| **Challenge Expiry** | 5-minute TTL prevents replay attacks |
| **Admin Rate Limiting** | Alert on >10 resets per hour |
| **Audit Logging** | All admin actions logged with reason |

### Admin Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/admin/webauthn/reset` | Reset student credential |
| POST | `/api/admin/webauthn/suspend` | Suspend credential (with reason) |
| POST | `/api/admin/webauthn/unsuspend` | Reactivate suspended credential |
| GET | `/api/admin/webauthn/credentials` | List all credentials (paginated) |
| GET | `/api/admin/webauthn/stats` | Enrollment statistics |

### Student Endpoints

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/s/:shortCode/webauthn/status/:rollNumber` | Check enrollment status |
| POST | `/s/:shortCode/webauthn/register/start` | Begin registration |
| POST | `/s/:shortCode/webauthn/register/finish` | Complete registration |
| POST | `/s/:shortCode/webauthn/authenticate/start` | Begin authentication |
| POST | `/s/:shortCode/webauthn/authenticate/finish` | Complete with attendance |

### Production Configuration

For production deployment, WebAuthn requires HTTPS:

```bash
WEBAUTHN_RP_NAME=Your Institution Name
WEBAUTHN_RP_ID=your-domain.com
WEBAUTHN_ORIGIN=https://your-domain.com
```

> **Note:** WebAuthn works on `localhost` for testing without HTTPS. In production, valid SSL certificate is required.

---

## Usage

### Admin Workflow

1. **Create Location**
   - Name: e.g., "Main Lecture Hall"
   - GPS coordinates (use Google Maps)
   - Radius: Geofence size in meters (default: 100m)

2. **Create Session**
   - Select location
   - Set duration (default: 30 minutes)
   - Optionally enable TOTP for additional security
   - System generates unique short link

3. **Share Link**
   - Copy link from session creation modal
   - Share via email/LMS/messaging
   - Display QR code for scanning

4. **Monitor Attendance**
   - View live stats
   - See verified/unverified students
   - Review flagged submissions
   - Manage WebAuthn credentials
   - Export to CSV

5. **Handle Issues**
   - Reset biometric credentials for students who lost devices
   - Suspend credentials for suspicious accounts
   - Rotate token if link gets shared

### Student Workflow

1. Open attendance link on mobile device
2. Enter roll number
3. **If first time:**
   - Complete biometric enrollment (Face ID/Touch ID)
   - Fill name
   - Capture photo
   - Enable GPS
   - Submit attendance
4. **If returning:**
   - Complete biometric verification
   - Capture new photo
   - Submit attendance

---

## API Endpoints

### Admin Endpoints (JWT Protected)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/api/admin/register` | Create admin (requires adminSecret) |
| POST | `/api/admin/login` | Login and get JWT token |
| POST | `/api/admin/locations` | Create location |
| GET | `/api/admin/locations` | List all locations |
| PUT | `/api/admin/locations/:id` | Update location |
| DELETE | `/api/admin/locations/:id` | Delete location |
| POST | `/api/admin/sessions` | Create session |
| GET | `/api/admin/sessions` | List all sessions |
| GET | `/api/admin/sessions/:id` | Get session details |
| POST | `/api/admin/sessions/:id/rotate` | Rotate session token |
| POST | `/api/admin/sessions/:id/deactivate` | Deactivate session |
| GET | `/api/admin/sessions/:id/attendance` | Get attendance records |
| GET | `/api/admin/sessions/:id/stats` | Get session statistics |
| GET | `/api/admin/sessions/:id/totp` | Get TOTP code |
| POST | `/api/admin/shortlinks` | Create short link |
| GET | `/api/admin/shortlinks` | List short links |
| POST | `/api/admin/shortlinks/:code/attach` | Attach link to session |
| DELETE | `/api/admin/shortlinks/:code` | Delete short link |
| POST | `/api/admin/webauthn/reset` | Reset biometric credential |
| POST | `/api/admin/webauthn/suspend` | Suspend credential |
| GET | `/api/admin/webauthn/credentials` | List credentials |
| GET | `/api/admin/webauthn/stats` | Enrollment statistics |

### Student Endpoints (Short Link Token)

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/s/:shortCode` | Redirect to student page |
| GET | `/s/:shortCode/info` | Get TOTP code |
| GET | `/s/:shortCode/session` | Get session info |
| GET | `/s/:shortCode/webauthn/status/:roll` | Check enrollment |
| POST | `/s/:shortCode/webauthn/register/start` | Begin registration |
| POST | `/s/:shortCode/webauthn/register/finish` | Complete registration |
| POST | `/s/:shortCode/webauthn/auth/start` | Begin authentication |
| POST | `/s/:shortCode/webauthn/auth/finish` | Complete with attendance |

---

## Development

### Backend

```bash
cd backend
npm install
npm run dev        # Development with hot reload
npm test           # Run tests (269 tests)
npm run test:watch # Watch mode
npm run lint       # Run ESLint
npm run lint:fix   # Fix linting issues
```

### Admin Frontend

```bash
cd frontend/admin
npm install
npm run dev        # Development server (port 5173)
npm run build      # Production build
npm run preview    # Preview production build
```

### Student Page

```bash
cd frontend/student/public
npx serve -p 8080
```

---

## Architecture

### Request Flow (with WebAuthn)

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Student Attendance Flow                         │
├─────────────────────────────────────────────────────────────────────┤
│  1. Student opens short link                                        │
│  2. GET /s/:shortCode → Redirect to student-scan.html               │
│  3. Enter roll number → GET webauthn/status                         │
│  4. If not enrolled:                                                 │
│     - POST webauthn/register/start → Get challenge                  │
│     - Browser prompts for biometric                                 │
│     - POST webauthn/register/finish → Store credential              │
│  5. If enrolled:                                                     │
│     - POST webauthn/authenticate/start → Get challenge              │
│     - Browser prompts for biometric                                 │
│     - POST webauthn/authenticate/finish → Verify + Submit           │
│  6. Photo capture, GPS check, Attendance saved                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Database Schema

```
Admins
  |-- username (unique)
  |-- email (unique)
  |-- password (bcrypt hashed)
  |-- role

Locations
  |-- name
  |-- latitude, longitude
  |-- radiusMeters
  |-- createdBy (ref: Admin)

Sessions
  |-- locationId (ref: Location)
  |-- tokenHash (SHA-256, unique)
  |-- tokenPrefix (first 8 chars)
  |-- expiresAt (TTL index)
  |-- isActive
  |-- totpEnabled
  |-- totpSecret (encrypted)
  |-- totpWindowSeconds

ShortLinks
  |-- shortCode (unique, lowercase)
  |-- sessionId (ref: Session)
  |-- createdBy (ref: Admin)
  |-- clickCount
  |-- isActive

Attendance
  |-- sessionId (ref: Session)
  |-- rollNumber (unique per session)
  |-- studentName
  |-- photoUrl, photoPublicId
  |-- studentLatitude, studentLongitude
  |-- distanceFromLocation
  |-- verified (boolean)
  |-- webauthnCredentialId (ref: WebAuthnCredential)
  |-- webauthnVerified (boolean)
  |-- webauthnDeviceType
  |-- webauthnCounter
  |-- flagged (boolean)
  |-- capturedAt

WebAuthnCredential
  |-- studentId (unique)
  |-- credentialId (unique)
  |-- publicKey (Buffer)
  |-- counter (sign count for replay detection)
  |-- deviceLabel
  |-- transports
  |-- aaguid (authenticator model)
  |-- isSuspended
  |-- suspendedReason
  |-- enrolledAt
  |-- lastUsedAt

WebAuthnChallenge
  |-- studentId
  |-- challenge
  |-- type (registration/authentication)
  |-- sessionId
  |-- used (boolean)
  |-- expiresAt (TTL: 5 minutes)

Device
  |-- fingerprintHash (SHA-256)
  |-- boundToStudent
  |-- sessionId
  |-- attendanceCount
  |-- flags[] (MULTI_STUDENT_DEVICE, etc.)
  |-- deviceFirstSeen

Flag
  |-- type
  |-- sessionId
  |-- attendanceId
  |-- reason
  |-- isRead
  |-- createdAt
```

---

## Environment Variables Reference

| Variable | Description | Required | Default |
|----------|-------------|----------|---------|
| `STORAGE_PROVIDER` | Storage backend: 'cloudinary' or 's3' | No | cloudinary |
| `CLOUDINARY_CLOUD_NAME` | Cloudinary cloud name | If cloudinary | - |
| `CLOUDINARY_API_KEY` | Cloudinary API key | If cloudinary | - |
| `CLOUDINARY_API_SECRET` | Cloudinary API secret | If cloudinary | - |
| `AWS_S3_BUCKET` | S3 bucket name | If s3 | - |
| `AWS_REGION` | AWS region | No | us-east-1 |
| `AWS_ACCESS_KEY_ID` | AWS access key | If s3 | - |
| `AWS_SECRET_ACCESS_KEY` | AWS secret key | If s3 | - |
| `JWT_SECRET` | JWT signing secret | Yes | - |
| `JWT_EXPIRE` | JWT expiration time | No | 7d |
| `ADMIN_SECRET` | Secret for admin creation | Yes | - |
| `MONGODB_URI` | MongoDB connection string | No | mongodb://localhost:27017/attendance-geotag |
| `WEBAUTHN_RP_NAME` | WebAuthn relying party name | No | Attendance System |
| `WEBAUTHN_RP_ID` | WebAuthn relying party ID (domain) | No | localhost |
| `WEBAUTHN_ORIGIN` | WebAuthn origin URL | No | http://localhost:5000 |
| `NGROK_AUTHTOKEN` | Ngrok auth token | Optional | - |
| `NGROK_DOMAIN` | Ngrok domain | Optional | - |

---

## Security Features

| Feature | Implementation |
|---------|---------------|
| **Biometric Verification** | WebAuthn with platform authenticators |
| **Replay Attack Prevention** | Sign counter tracking |
| **Challenge Expiry** | 5-minute TTL for WebAuthn challenges |
| **Token Hashing** | Session tokens stored as SHA-256 hashes |
| **JWT Authentication** | Admin routes protected with JWT |
| **Password Hashing** | bcrypt with 10 salt rounds |
| **Rate Limiting** | Admin: 100/15min, Student: 20/min, Login: 5/15min |
| **Input Validation** | express-validator on all inputs |
| **CORS** | Configurable origins |
| **Security Headers** | Helmet middleware |
| **S3 Presigned URLs** | Time-limited (5 min) upload URLs |
| **MongoDB Injection** | Mongoose sanitization |
| **Device Fingerprinting** | Detect suspicious device activity |
| **Admin Audit Trail** | All credential changes logged |

---

## Testing

### Test Coverage

| Category | Tests | Status |
|----------|-------|--------|
| WebAuthn Flow | 66 | ✅ Passing |
| TOTP Utilities | 15 | ✅ Passing |
| ShortLink API | 25 | ✅ Passing |
| Device Model | 8 | ✅ Passing |
| Session/TOTP | 12 | ✅ Passing |
| Models | 28 | ✅ Passing |
| Middleware | 38 | ✅ Passing |
| Attendance | 45 | ✅ Passing |
| Security | 12 | ✅ Passing |
| **Total** | **269** | **✅ 100% Passing** |

### Running Tests

```bash
# Backend tests
cd backend
npm test                    # Run all tests
npm test -- --coverage      # With coverage report
npm test -- tests/webauthn  # WebAuthn tests only

# Frontend tests
cd frontend/admin
npm test
```

### CI/CD Pipeline

GitHub Actions workflow includes:
- Backend lint and test
- Frontend lint and build
- Security scanning with CodeQL
- Docker image build and push

---

## Scaling

### For 1000+ Concurrent Users

| Optimization | Implementation |
|--------------|----------------|
| MongoDB Connection Pooling | 300 max connections |
| Database Indexing | Unique index on `{sessionId, rollNumber}` |
| S3 Direct Upload | Reduces backend load significantly |
| Cloudinary CDN | Images served from global CDN |
| Rate Limiting | Prevents abuse |
| WebAuthn Challenge TTL | Auto-expiry prevents accumulation |

### Performance Metrics

| Metric | Value |
|--------|-------|
| Max RPS (backend) | ~150-250 |
| Photo upload latency (S3) | 200-400ms |
| Photo upload latency (Cloudinary) | 500-1500ms |
| WebAuthn verification | 100-300ms |
| MongoDB query time | <10ms (indexed) |
| Token validation | <5ms |

---

## Hardware Requirements

### Minimum (1,000 students)

| Component | Specification |
|-----------|---------------|
| CPU | 2 vCPUs (t3.medium) |
| RAM | 4GB |
| Storage | 20GB SSD |
| MongoDB | Atlas M10 or 2GB self-hosted |
| Estimated Cost | ~$87/month |

### Recommended Production (5,000 students)

| Component | Specification |
|-----------|---------------|
| CPU | 4 vCPUs (c5.large) |
| RAM | 8GB |
| Storage | 50GB SSD |
| MongoDB | Atlas M20 |
| Estimated Cost | ~$150/month |

---

## Project Structure

```
Attendence-GEOTAG-System/
|-- .github/
|   `-- workflows/
|       |-- ci.yml              # CI/CD pipeline
|       `-- codeql.yml          # Security scanning
|
|-- backend/
|   |-- src/
|   |   |-- config/             # Configuration files
|   |   |-- controllers/        # Route controllers
|   |   |-- middleware/         # Auth, validation, rate limiting
|   |   |-- models/             # Mongoose models
|   |   |-- routes/             # Express routes
|   |   |-- storage/            # Storage providers (Cloudinary, S3)
|   |   |-- utils/              # Utility functions (WebAuthn, TOTP)
|   |   `-- server.js           # Entry point
|   |-- tests/                  # Test files (269 tests)
|   |-- jest.config.js
|   |-- jest.setup.js
|   `-- package.json
|
|-- frontend/
|   |-- admin/                  # React admin panel
|   |   |-- src/
|   |   |   |-- components/     # React components
|   |   |   |-- context/        # Auth context
|   |   |   |-- pages/          # Page components
|   |   |   `-- index.css       # Global styles
|   |   |-- public/
|   |   `-- package.json
|   |
|   `-- student/                # Static student page
|       `-- public/
|           `-- student-scan.html
|
|-- docker-compose.yml
|-- .env.example
|-- HardwareRequirements.md
`-- README.md
```

---

## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Commit changes: `git commit -am 'Add my feature'`
4. Push to branch: `git push origin feature/my-feature`
5. Submit a pull request

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
