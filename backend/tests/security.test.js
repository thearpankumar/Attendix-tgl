const request = require('supertest');
const Session = require('../src/models/Session');
const Admin = require('../src/models/Admin');
const Location = require('../src/models/Location');
const Attendance = require('../src/models/Attendance');
const crypto = require('crypto');

// MongoMemoryServer lifecycle is handled by globalSetup.js + dbSetup.js
let app;

beforeAll(async () => {
  app = require('../src/server');
});

beforeEach(async () => {
  await Session.deleteMany({});
  await Admin.deleteMany({});
  await Location.deleteMany({});
  await Attendance.deleteMany({});
});

describe('Security Tests', () => {
  let adminToken, admin, location, session, attendanceToken;

  beforeEach(async () => {
    const adminRes = await request(app)
      .post('/api/admin/register')
      .send({
        username: 'secadmin',
        email: 'sec@example.com',
        password: 'SecurePass123!',
        adminSecret: 'test-admin-secret'
      });

    // Assert loudly — if this fails every dependent test fails with a clear
    // message instead of silently passing with undefined tokens.
    expect(adminRes.status).toBe(201);
    adminToken = adminRes.body.token;
    admin = adminRes.body.admin;

    const locationRes = await request(app)
      .post('/api/admin/locations')
      .set('Authorization', `Bearer ${adminToken}`)
      .send({
        name: 'Security Test Location',
        latitude: 12.9716,
        longitude: 77.5946,
        radiusMeters: 100
      });

    expect(locationRes.status).toBe(201);
    location = locationRes.body;

    const sessionRes = await request(app)
      .post('/api/admin/sessions')
      .set('Authorization', `Bearer ${adminToken}`)
      .send({
        locationId: location._id,
        durationMinutes: 30,
        description: 'Security Test Session'
      });

    expect(sessionRes.status).toBe(201);
    session = sessionRes.body;
    attendanceToken = session.token;
  });

  describe('Authentication Security', () => {
    it('should reject invalid JWT tokens', async () => {
      await request(app)
        .get('/api/admin/sessions')
        .set('Authorization', 'Bearer invalidtoken123')
        .expect(401);
    });

    it('should reject malformed JWT tokens', async () => {
      await request(app)
        .get('/api/admin/sessions')
        .set('Authorization', 'Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.invalid.signature')
        .expect(401);
    });

    it('should reject requests without Authorization header', async () => {
      await request(app)
        .get('/api/admin/sessions')
        .expect(401);
    });

    it('should handle token tampering', async () => {
      const tamperedToken = adminToken.slice(0, -5) + 'xxxxx';
      
      await request(app)
        .get('/api/admin/sessions')
        .set('Authorization', `Bearer ${tamperedToken}`)
        .expect(401);
    });

    it('should reject weak passwords', async () => {
      const res = await request(app)
        .post('/api/admin/register')
        .send({
          username: 'weakpassuser',
          email: 'weak@example.com',
          password: '123',
          adminSecret: 'test-admin-secret'
        });

      expect(res.status).toBeGreaterThanOrEqual(400);
    });

    it('should require admin secret for registration', async () => {
      await request(app)
        .post('/api/admin/register')
        .send({
          username: 'nosecretadmin',
          email: 'nosecret@example.com',
          password: 'password123'
        })
        .expect(403);
    });

    it('should prevent duplicate admin usernames', async () => {
      await request(app)
        .post('/api/admin/register')
        .send({
          username: 'secadmin',
          email: 'duplicate@example.com',
          password: 'password123',
          adminSecret: 'test-admin-secret'
        })
        .expect(400);
    });

    it('should prevent duplicate admin emails', async () => {
      await request(app)
        .post('/api/admin/register')
        .send({
          username: 'newusername',
          email: 'sec@example.com',
          password: 'password123',
          adminSecret: 'test-admin-secret'
        })
        .expect(400);
    });
  });

  describe('Token Security', () => {
    it('should use SHA-256 for token hashing', () => {
      const token = 'testtoken1234567890';
      const hash1 = Session.hashToken(token);
      const hash2 = Session.hashToken(token);

      expect(hash1).toBe(hash2);
      expect(hash1).toHaveLength(64);
      expect(/^[a-f0-9]{64}$/.test(hash1)).toBe(true);
    });

    it('should generate unique tokens', () => {
      const tokens = new Set();
      for (let i = 0; i < 100; i++) {
        tokens.add(Session.generateToken());
      }
      expect(tokens.size).toBe(100);
    });

    it('should store only token hash, not plain token', async () => {
      const dbSession = await Session.findById(session._id);
      
      expect(dbSession.tokenHash).toBeDefined();
      expect(dbSession.tokenHash).not.toBe(attendanceToken);
      expect(dbSession.tokenHash).toHaveLength(64);
    });

    it('should invalidate old token after rotation', async () => {
      const oldToken = attendanceToken;

      await request(app)
        .post(`/api/admin/sessions/${session._id}/rotate`)
        .set('Authorization', `Bearer ${adminToken}`)
        .expect(200);

      await request(app)
        .get(`/api/attend/${oldToken}`)
        .expect(404);
    });

    it('should prevent token prediction', async () => {
      const tokens = [];
      for (let i = 0; i < 10; i++) {
        const token = Session.generateToken();
        tokens.push(token);
      }

      for (let i = 1; i < tokens.length; i++) {
        expect(tokens[i]).not.toBe(tokens[i-1]);
        expect(tokens[i].substring(0, 10)).not.toBe(tokens[i-1].substring(0, 10));
      }
    });
  });

  describe('Input Validation', () => {
    it('should sanitize SQL injection attempts', async () => {
      const res = await request(app)
        .post('/api/admin/register')
        .send({
          username: "admin' OR '1'='1",
          email: "test@example.com",
          password: 'password123',
          adminSecret: 'test-admin-secret'
        });

      expect(res.status).toBeGreaterThanOrEqual(400);
    });

    it('should sanitize XSS attempts in location name', async () => {
      const res = await request(app)
        .post('/api/admin/locations')
        .set('Authorization', `Bearer ${adminToken}`)
        .send({
          name: '<script>alert("XSS")</script>',
          latitude: 12.9716,
          longitude: 77.5946,
          radiusMeters: 100
        });

      // The validator escapes HTML so the request succeeds (201)
      // but the stored name must NOT contain raw script tags.
      expect(res.status).toBe(201);
      expect(res.body.name).not.toContain('<script>');
      expect(res.body.name).not.toContain('</script>');
      // Verify the name was actually HTML-encoded (not just stripped)
      expect(res.body.name).toContain('&lt;script&gt;');
    });

    it('should validate latitude range', async () => {
      const res = await request(app)
        .post('/api/admin/locations')
        .set('Authorization', `Bearer ${adminToken}`)
        .send({
          name: 'Invalid Location',
          latitude: 999,
          longitude: 77.5946,
          radiusMeters: 100
        });

      expect(res.status).toBeGreaterThanOrEqual(400);
    });

    it('should validate longitude range', async () => {
      const res = await request(app)
        .post('/api/admin/locations')
        .set('Authorization', `Bearer ${adminToken}`)
        .send({
          name: 'Invalid Location',
          latitude: 12.9716,
          longitude: 999,
          radiusMeters: 100
        });

      expect(res.status).toBeGreaterThanOrEqual(400);
    });

    it('should handle negative radius', async () => {
      const res = await request(app)
        .post('/api/admin/locations')
        .set('Authorization', `Bearer ${adminToken}`)
        .send({
          name: 'Invalid Radius',
          latitude: 12.9716,
          longitude: 77.5946,
          radiusMeters: -100
        });

      expect(res.status).toBeGreaterThanOrEqual(400);
    });

    it('should handle missing required fields', async () => {
      await request(app)
        .post('/api/admin/locations')
        .set('Authorization', `Bearer ${adminToken}`)
        .send({
          name: 'Missing Fields'
        })
        .expect(400);
    });
  });

  describe('Access Control', () => {
    let otherAdmin, otherAdminToken;

    beforeEach(async () => {
      await request(app)
        .post('/api/admin/register')
        .send({
          username: 'otheradmin',
          email: 'other@example.com',
          password: 'password123',
          adminSecret: 'test-admin-secret'
        });

      const loginRes = await request(app)
        .post('/api/admin/login')
        .send({
          username: 'otheradmin',
          password: 'password123'
        });

      otherAdminToken = loginRes.body.token;
    });

    it('should prevent admin from accessing others sessions', async () => {
      await request(app)
        .get(`/api/admin/sessions/${session._id}`)
        .set('Authorization', `Bearer ${otherAdminToken}`)
        .expect(404);
    });

    it('should prevent admin from rotating others tokens', async () => {
      await request(app)
        .post(`/api/admin/sessions/${session._id}/rotate`)
        .set('Authorization', `Bearer ${otherAdminToken}`)
        .expect(404);
    });

    it('should prevent admin from deleting others sessions', async () => {
      await request(app)
        .delete(`/api/admin/sessions/${session._id}`)
        .set('Authorization', `Bearer ${otherAdminToken}`)
        .expect(404);
    });
  });

  describe('Rate Limiting Security', () => {
    it('should handle rapid requests without crashing', async () => {
      const promises = Array(30).fill(null).map(() =>
        request(app).get('/health')
      );

      const results = await Promise.all(promises);
      const successCount = results.filter(r => r.status === 200).length;
      expect(successCount).toBeGreaterThan(25);
    });

    it('should reject all brute force login attempts with wrong password', async () => {
      const promises = Array(10).fill(null).map(() =>
        request(app)
          .post('/api/admin/login')
          .send({
            username: 'secadmin',
            password: 'wrongpassword'
          })
      );

      const results = await Promise.all(promises);
      // Every attempt uses wrong credentials → all must be rejected (401).
      // Rate limiter is skipped in test mode, so all 10 reach the auth logic.
      const failureCount = results.filter(r => r.status === 401).length;
      expect(failureCount).toBe(10);
      // Sanity: none should have succeeded
      const successCount = results.filter(r => r.status === 200).length;
      expect(successCount).toBe(0);
    });
  });

  describe('Data Privacy', () => {
    it('should not expose passwords in API responses', async () => {
      const res = await request(app)
        .post('/api/admin/register')
        .send({
          username: 'privacyuser',
          email: 'privacy@example.com',
          password: 'SecurePass123!',
          adminSecret: 'test-admin-secret'
        });

      expect(res.body.admin.password).toBeUndefined();
      expect(res.body.admin).toBeDefined();
    });

    it('should not expose sensitive fields in session listing', async () => {
      const res = await request(app)
        .get('/api/admin/sessions')
        .set('Authorization', `Bearer ${adminToken}`);

      res.body.forEach(session => {
        expect(session.totpSecret).toBeUndefined();
        expect(session.createdBy).toBeDefined();
      });
    });

    it('should not expose internal token hash', async () => {
      const res = await request(app)
        .get(`/api/attend/${attendanceToken}`)
        .expect(200);

      expect(res.body.session.tokenHash).toBeUndefined();
    });
  });

  describe('Geospatial Security', () => {
    it('should accept attendance from nearby location and mark verified=true', async () => {
      // Within 100m radius of Security Test Location (12.9716, 77.5946)
      const nearbyLat = 12.9720;
      const nearbyLon = 77.5950;

      const res = await request(app)
        .post(`/api/attend/${attendanceToken}`)
        .send({
          studentName: 'Test Student',
          rollNumber: 'TEST001',
          photo: 'data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQ',
          latitude: nearbyLat,
          longitude: nearbyLon,
          faceDetected: true
        });

      // Upload is mocked, so this must succeed.
      expect(res.status).toBe(201);
      // Student is within the geofence radius → verified must be true.
      expect(res.body.attendance.verified).toBe(true);
      expect(res.body.attendance.distanceFromLocation).toBeLessThanOrEqual(100);
    });

    it('should accept attendance from far location but mark verified=false', async () => {
      // ~17 km away — outside the 100m radius.
      const farLat = 13.0;
      const farLon = 78.0;

      const res = await request(app)
        .post(`/api/attend/${attendanceToken}`)
        .send({
          studentName: 'Far Student',
          rollNumber: 'FAR001',
          photo: 'data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQ',
          latitude: farLat,
          longitude: farLon,
          faceDetected: true
        });

      // Upload succeeds (mocked) but location check marks verified=false.
      // The controller still creates the record — it does NOT reject it.
      expect(res.status).toBe(201);
      expect(res.body.attendance.verified).toBe(false);
      expect(res.body.attendance.distanceFromLocation).toBeGreaterThan(100);
    });
  });

  describe('Error Information Leakage', () => {
    it('should not leak stack traces in production', async () => {
      const originalEnv = process.env.NODE_ENV;
      process.env.NODE_ENV = 'production';

      const res = await request(app)
        .post('/api/admin/register')
        .send({
          username: 'erroruser',
          email: 'invalid-email',   // triggers validation error
          password: 'password123',
          adminSecret: 'test-admin-secret'
        });

      // Always assert — no conditional branch.
      // Validation errors should return 400 (never 500 with a stack trace).
      expect(res.status).toBe(400);
      expect(res.body.stack).toBeUndefined();
      expect(res.body.message).toBeDefined();

      process.env.NODE_ENV = originalEnv;
    });

    it('should return generic error messages', async () => {
      const res = await request(app)
        .get('/api/admin/sessions/invalidid123');

      expect(res.body.message).toBeDefined();
      expect(res.body.message).not.toContain('Error:');
    });
  });

  describe('Session Management Security', () => {
    it('should auto-expire sessions', async () => {
      await Session.findByIdAndUpdate(session._id, {
        expiresAt: new Date(Date.now() - 1000)
      });

      await request(app)
        .get(`/api/attend/${attendanceToken}`)
        .expect(404);
    });

    it('should prevent reuse of deactivated session token', async () => {
      await request(app)
        .post(`/api/admin/sessions/${session._id}/deactivate`)
        .set('Authorization', `Bearer ${adminToken}`)
        .expect(200);

      await request(app)
        .get(`/api/attend/${attendanceToken}`)
        .expect(404);
    });
  });

  describe('TOTP Security', () => {
    it('should generate time-based codes', async () => {
      const dbSession = await Session.findById(session._id);
      
      expect(dbSession.totpEnabled).toBeDefined();
      
      if (dbSession.totpEnabled) {
        const code = dbSession.generateTOTP();
        expect(code).toHaveLength(6);
        expect(/^[A-Z0-9]{6}$/.test(code)).toBe(true);
      }
    });

    it('should validate TOTP within tolerance', async () => {
      const dbSession = await Session.findById(session._id);
      
      if (dbSession.totpEnabled) {
        const code = dbSession.generateTOTP();
        const validation = dbSession.validateTOTP(code);
        expect(validation.valid).toBe(true);
      }
    });
  });
});

describe('Redis Cache Security', () => {
  it('should handle cache gracefully when Redis is unavailable', async () => {
    const { getCachedSession } = require('../src/middleware/sessionCache');
    
    const fakeHash = 'nonexistent12345678901234567890123456789012';
    const result = await getCachedSession(fakeHash);
    expect(result).toBeNull();
  });
});

describe('Concurrent Access Security', () => {
  it('should handle concurrent session accesses safely', async () => {
    const adminRes = await request(app)
      .post('/api/admin/register')
      .send({
        username: 'concadmin',
        email: 'conc@example.com',
        password: 'password123',
        adminSecret: 'test-admin-secret'
      });

    const token = adminRes.body.token;

    const locationRes = await request(app)
      .post('/api/admin/locations')
      .set('Authorization', `Bearer ${token}`)
      .send({
        name: 'Concurrent Location',
        latitude: 12.9716,
        longitude: 77.5946,
        radiusMeters: 100
      });

    const promises = Array(5).fill(null).map(() =>
      request(app)
        .post('/api/admin/sessions')
        .set('Authorization', `Bearer ${token}`)
        .send({
          locationId: locationRes.body._id,
          durationMinutes: 30
        })
    );

    const results = await Promise.all(promises);
    const successCount = results.filter(r => r.status === 201).length;
    expect(successCount).toBe(5);

    const tokens = results.map(r => r.body.token);
    const uniqueTokens = new Set(tokens);
    expect(uniqueTokens.size).toBe(5);
  });
});
