const request = require('supertest');
const Session = require('../src/models/Session');
const Admin = require('../src/models/Admin');
const Location = require('../src/models/Location');

// MongoMemoryServer lifecycle is handled by globalSetup.js + dbSetup.js
let app;

beforeAll(async () => {
  app = require('../src/server');
});

beforeEach(async () => {
  await Session.deleteMany({});
  await Admin.deleteMany({});
  await Location.deleteMany({});
});

describe('System Integration Tests', () => {
  let adminToken, admin, location, session, attendanceToken;

  beforeEach(async () => {
    const adminRes = await request(app)
      .post('/api/admin/register')
      .send({
        username: 'testadmin',
        email: 'test@example.com',
        password: 'password123',
        adminSecret: 'test-admin-secret'
      });
    
    adminToken = adminRes.body.token;
    admin = adminRes.body.admin;

    const locationRes = await request(app)
      .post('/api/admin/locations')
      .set('Authorization', `Bearer ${adminToken}`)
      .send({
        name: 'Test Location',
        latitude: 12.9716,
        longitude: 77.5946,
        radiusMeters: 100
      });

    location = locationRes.body;

    const sessionRes = await request(app)
      .post('/api/admin/sessions')
      .set('Authorization', `Bearer ${adminToken}`)
      .send({
        locationId: location._id,
        durationMinutes: 30,
        description: 'Test Session'
      });

    session = sessionRes.body;
    attendanceToken = session.token;
  });

  describe('Health Endpoints', () => {
    it('should return OK status for /health', async () => {
      const res = await request(app)
        .get('/health')
        .expect(200);

      expect(res.body.status).toBe('OK');
      expect(res.body.timestamp).toBeDefined();
    });

    it('should return ready status for /health/ready', async () => {
      const res = await request(app)
        .get('/health/ready')
        .expect(200);

      expect(res.body.status).toBe('ready');
      expect(res.body.redis).toBeDefined();
    });

    it('should return alive status for /health/live', async () => {
      const res = await request(app)
        .get('/health/live')
        .expect(200);

      expect(res.body.status).toBe('alive');
    });
  });

  describe('Storage Configuration', () => {
    it('should return storage provider info', async () => {
      const res = await request(app)
        .get('/api/storage-info')
        .expect(200);

      // In test env STORAGE_PROVIDER=cloudinary (forced by tests/setup.js).
      // Production uses s3 — this test validates the endpoint shape, not the
      // specific provider name which is deployment-config dependent.
      expect(res.body.provider).toBeDefined();
      expect(['cloudinary', 's3']).toContain(res.body.provider);
      expect(typeof res.body.supportsDirectUpload).toBe('boolean');
    });
  });

  describe('Session Flow with Caching', () => {
    it('should validate attendance token quickly', async () => {
      const start = Date.now();
      
      const res = await request(app)
        .get(`/api/attend/${attendanceToken}`)
        .expect(200);

      const duration = Date.now() - start;
      expect(duration).toBeLessThan(100);
      expect(res.body.valid).toBe(true);
      expect(res.body.session.locationName).toBe('Test Location');
    });

    it('should handle multiple simultaneous validations', async () => {
      const promises = Array(5).fill(null).map(() =>
        request(app)
          .get(`/api/attend/${attendanceToken}`)
          .expect(200)
      );

      const results = await Promise.all(promises);
      results.forEach(res => {
        expect(res.body.valid).toBe(true);
      });
    });

    it('should invalidate cache on token rotation', async () => {
      const oldToken = attendanceToken;

      const rotateRes = await request(app)
        .post(`/api/admin/sessions/${session._id}/rotate`)
        .set('Authorization', `Bearer ${adminToken}`)
        .expect(200);

      const newToken = rotateRes.body.token;
      expect(newToken).not.toBe(oldToken);

      await request(app)
        .get(`/api/attend/${oldToken}`)
        .expect(404);

      await request(app)
        .get(`/api/attend/${newToken}`)
        .expect(200);
    });
  });

  describe('Error Scenarios', () => {
    it('should handle invalid token format gracefully', async () => {
      const res = await request(app)
        .get('/api/attend/invalidtoken')
        .expect(404);

      expect(res.body.valid).toBe(false);
    });

    it('should handle missing token', async () => {
      const res = await request(app)
        .get('/api/attend/')
        .expect(404);
    });

    it('should reject expired sessions', async () => {
      await Session.findByIdAndUpdate(session._id, {
        expiresAt: new Date(Date.now() - 1000)
      });

      const res = await request(app)
        .get(`/api/attend/${attendanceToken}`)
        .expect(404);

      expect(res.body.valid).toBe(false);
    });

    it('should reject deactivated sessions', async () => {
      await request(app)
        .post(`/api/admin/sessions/${session._id}/deactivate`)
        .set('Authorization', `Bearer ${adminToken}`)
        .expect(200);

      const res = await request(app)
        .get(`/api/attend/${attendanceToken}`)
        .expect(404);

      expect(res.body.valid).toBe(false);
    });
  });

  describe('Rate Limiting', () => {
    it('should handle rapid requests without crashing', async () => {
      const promises = Array(25).fill(null).map((_, i) =>
        request(app)
          .get('/health')
      );

      const results = await Promise.all(promises);
      const successCount = results.filter(r => r.status === 200).length;
      expect(successCount).toBeGreaterThan(20);
    });
  });

  describe('Admin Operations', () => {
    it('should list sessions with pagination', async () => {
      const res = await request(app)
        .get('/api/admin/sessions')
        .set('Authorization', `Bearer ${adminToken}`)
        .expect(200);

      expect(Array.isArray(res.body)).toBe(true);
      expect(res.body.length).toBeGreaterThan(0);
      // sessions listing populates locationId as an object
      expect(res.body[0].locationId).toBeDefined();
      expect(res.body[0].attendanceCount).toBeDefined();
    });

    it('should get session details', async () => {
      const res = await request(app)
        .get(`/api/admin/sessions/${session._id}`)
        .set('Authorization', `Bearer ${adminToken}`)
        .expect(200);

      // locationId is populated as an object with name, lat, lng etc.
      expect(res.body.locationId).toBeDefined();
      expect(res.body.attendanceCount).toBe(0);
    });

    it('should reject unauthorized access', async () => {
      await request(app)
        .get('/api/admin/sessions')
        .expect(401);
    });
  });
});

describe('Load Testing Scenarios', () => {
  let adminToken, location;

  beforeEach(async () => {
    const adminRes = await request(app)
      .post('/api/admin/register')
      .send({
        username: 'loadtest',
        email: 'load@example.com',
        password: 'password123',
        adminSecret: 'test-admin-secret'
      });
    
    adminToken = adminRes.body.token;

    const locationRes = await request(app)
      .post('/api/admin/locations')
      .set('Authorization', `Bearer ${adminToken}`)
      .send({
        name: 'Load Test Location',
        latitude: 12.9716,
        longitude: 77.5946,
        radiusMeters: 100
      });

    location = locationRes.body;
  });

  it('should handle 100 concurrent health checks', async () => {
    const promises = Array(100).fill(null).map(() =>
      request(app).get('/health')
    );

    const start = Date.now();
    const results = await Promise.all(promises);
    const duration = Date.now() - start;

    const successCount = results.filter(r => r.status === 200).length;
    expect(successCount).toBe(100);
    expect(duration).toBeLessThan(2000);
  });

  it('should handle concurrent session creations', async () => {
    const promises = Array(10).fill(null).map(() =>
      request(app)
        .post('/api/admin/sessions')
        .set('Authorization', `Bearer ${adminToken}`)
        .send({
          locationId: location._id,
          durationMinutes: 30
        })
    );

    const results = await Promise.all(promises);
    const successCount = results.filter(r => r.status === 201).length;
    expect(successCount).toBe(10);
  });
});

describe('Edge Cases', () => {
  it('should handle very long token strings', async () => {
    const longToken = 'a'.repeat(1000);
    
    const res = await request(app)
      .get(`/api/attend/${longToken}`)
      .expect(404);
  });

  it('should handle special characters in token', async () => {
    const specialToken = 'token-with-special-chars!@#$%';
    
    const res = await request(app)
      .get(`/api/attend/${specialToken}`)
      .expect(404);
  });

  it('should handle concurrent admin logins', async () => {
    const password = 'password123';
    
    await request(app)
      .post('/api/admin/register')
      .send({
        username: 'concuser',
        email: 'conc@example.com',
        password,
        adminSecret: 'test-admin-secret'
      });

    const promises = Array(5).fill(null).map(() =>
      request(app)
        .post('/api/admin/login')
        .send({
          username: 'concuser',
          password
        })
    );

    const results = await Promise.all(promises);
    const successCount = results.filter(r => r.status === 200).length;
    expect(successCount).toBe(5);
  });
});
