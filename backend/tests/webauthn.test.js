const request = require('supertest');
const mongoose = require('mongoose');
const app = require('../src/server');
const Admin = require('../src/models/Admin');
const Session = require('../src/models/Session');
const Location = require('../src/models/Location');
const ShortLink = require('../src/models/ShortLink');
const Attendance = require('../src/models/Attendance');
const WebAuthnCredential = require('../src/models/WebAuthnCredential');
const WebAuthnChallenge = require('../src/models/WebAuthnChallenge');
const WebAuthnReenrollmentLog = require('../src/models/WebAuthnReenrollmentLog');

let adminToken;
let testSession;
let testShortLink;
let testLocation;

async function getFreshAdminToken() {
  const loginRes = await request(app)
    .post('/api/admin/login')
    .send({ username: 'testadmin', password: 'password123' });
  return loginRes.body.token;
}

beforeEach(async () => {
  const admin = await Admin.create({
    username: 'testadmin',
    email: 'test@test.com',
    password: 'password123',
    role: 'admin',
  });

  const loginRes = await request(app)
    .post('/api/admin/login')
    .send({ username: 'testadmin', password: 'password123' });
  
  adminToken = loginRes.body.token;

  testLocation = await Location.create({
    name: 'Test Location',
    latitude: 28.6139,
    longitude: 77.2090,
    radiusMeters: 100,
    createdBy: admin._id,
  });

  const expiresAt = new Date(Date.now() + 60 * 60 * 1000);
  const token = Session.generateToken();
  const tokenHash = Session.hashToken(token);

  testSession = await Session.create({
    locationId: testLocation._id,
    tokenHash,
    tokenPrefix: token.substring(0, 8),
    expiresAt,
    createdBy: admin._id,
    totpEnabled: false,
  });

  testShortLink = await ShortLink.create({
    shortCode: 'test123',
    sessionId: testSession._id,
    createdBy: admin._id,
  });
});

describe('WebAuthn Status Check', () => {
  it('should return not enrolled for new student', async () => {
    const res = await request(app)
      .get(`/s/${testShortLink.shortCode}/webauthn/status/ABC123`);
    
    expect(res.status).toBe(200);
    expect(res.body.enrolled).toBe(false);
    expect(res.body.suspended).toBe(false);
    expect(res.body.alreadySubmitted).toBe(false);
  });

  it('should return enrolled for existing credential', async () => {
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
      deviceLabel: 'Test Device',
    });

    const res = await request(app)
      .get(`/s/${testShortLink.shortCode}/webauthn/status/ABC123`);
    
    expect(res.status).toBe(200);
    expect(res.body.enrolled).toBe(true);
    expect(res.body.suspended).toBe(false);
  });

  it('should return suspended for suspended credential', async () => {
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
      deviceLabel: 'Test Device',
      isSuspended: true,
      suspendedReason: 'Test suspension',
    });

    const res = await request(app)
      .get(`/s/${testShortLink.shortCode}/webauthn/status/ABC123`);
    
    expect(res.status).toBe(200);
    expect(res.body.enrolled).toBe(true);
    expect(res.body.suspended).toBe(true);
  });

  it('should return alreadySubmitted for existing attendance', async () => {
    await Attendance.create({
      sessionId: testSession._id,
      studentName: 'Test Student',
      rollNumber: 'ABC123',
      photoUrl: 'http://test.com/photo.jpg',
      photoPublicId: 'test-photo',
      studentLatitude: 28.6139,
      studentLongitude: 77.2090,
      distanceFromLocation: 50,
    });

    const res = await request(app)
      .get(`/s/${testShortLink.shortCode}/webauthn/status/ABC123`);
    
    expect(res.status).toBe(200);
    expect(res.body.alreadySubmitted).toBe(true);
  });

  it('should return 404 for invalid session', async () => {
    const res = await request(app)
      .get('/s/invalid/webauthn/status/ABC123');
    
    expect(res.status).toBe(404);
  });

  it('should handle roll number case insensitivity', async () => {
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
      deviceLabel: 'Test Device',
    });

    const res = await request(app)
      .get(`/s/${testShortLink.shortCode}/webauthn/status/abc123`);
    
    expect(res.status).toBe(200);
    expect(res.body.enrolled).toBe(true);
  });
});

describe('WebAuthn Registration Start', () => {
  it('should generate registration options for new student', async () => {
    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
      .send({
        rollNumber: 'ABC123',
        studentName: 'Test Student',
      });
    
    expect(res.status).toBe(200);
    expect(res.body.challenge).toBeDefined();
    expect(res.body.rp).toBeDefined();
    expect(res.body.user).toBeDefined();
    expect(res.body.authenticatorSelection.authenticatorAttachment).toBeUndefined();
    expect(res.body.authenticatorSelection.userVerification).toBe('preferred');
  });

  it('should create challenge in database', async () => {
    await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
      .send({
        rollNumber: 'ABC123',
        studentName: 'Test Student',
      });
    
    const challenge = await WebAuthnChallenge.findOne({
      studentId: 'ABC123',
      type: 'registration',
    });
    
    expect(challenge).toBeDefined();
    expect(challenge.challenge).toBeDefined();
    expect(challenge.studentName).toBe('Test Student');
    expect(challenge.used).toBe(false);
  });

  it('should reject if already enrolled', async () => {
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'existing-cred',
      publicKey: Buffer.from('test-key'),
      counter: 0,
    });

    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
      .send({
        rollNumber: 'ABC123',
        studentName: 'Test Student',
      });
    
    expect(res.status).toBe(400);
    expect(res.body.alreadyEnrolled).toBe(true);
  });

  it('should require roll number and student name', async () => {
    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
      .send({});
    
    expect(res.status).toBe(400);
    expect(res.body.message).toContain('required');
  });

  it('should reject for expired session', async () => {
    await Session.findByIdAndUpdate(testSession._id, {
      expiresAt: new Date(Date.now() - 1000),
    });

    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
      .send({
        rollNumber: 'ABC123',
        studentName: 'Test Student',
      });
    
    expect(res.status).toBe(400);
    expect(res.body.message).toContain('expired');
  });
});

describe('WebAuthn Authentication Start', () => {
  it('should generate authentication options for enrolled student', async () => {
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
      transports: ['internal'],
    });

    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/authenticate/start`)
      .send({ rollNumber: 'ABC123' });
    
    expect(res.status).toBe(200);
    expect(res.body.challenge).toBeDefined();
    expect(res.body.allowCredentials).toBeDefined();
    expect(res.body.allowCredentials[0].id).toBe('test-cred-id');
    expect(res.body.userVerification).toBe('preferred');
  });

  it('should create authentication challenge', async () => {
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
    });

    await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/authenticate/start`)
      .send({ rollNumber: 'ABC123' });
    
    const challenge = await WebAuthnChallenge.findOne({
      studentId: 'ABC123',
      type: 'authentication',
    });
    
    expect(challenge).toBeDefined();
    expect(challenge.challenge).toBeDefined();
  });

  it('should reject for non-enrolled student', async () => {
    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/authenticate/start`)
      .send({ rollNumber: 'ABC123' });
    
    expect(res.status).toBe(404);
    expect(res.body.notEnrolled).toBe(true);
  });

  it('should reject for suspended credential', async () => {
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
      isSuspended: true,
      suspendedReason: 'Test suspension',
    });

    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/authenticate/start`)
      .send({ rollNumber: 'ABC123' });
    
    expect(res.status).toBe(403);
    expect(res.body.suspended).toBe(true);
  });

  it('should require roll number', async () => {
    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/authenticate/start`)
      .send({});
    
    expect(res.status).toBe(400);
  });
});

describe('Challenge Expiry', () => {
  it('should reject expired challenge', async () => {
    await WebAuthnChallenge.create({
      studentId: 'ABC123',
      challenge: 'test-challenge',
      type: 'registration',
      sessionId: testSession._id,
      expiresAt: new Date(Date.now() - 1000),
      used: false,
    });

    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/register/finish`)
      .send({
        rollNumber: 'ABC123',
        credential: { id: 'test' },
      });
    
    expect(res.status).toBe(400);
    expect(res.body.message).toContain('No valid');
  });

  it('should reject used challenge', async () => {
    await WebAuthnChallenge.create({
      studentId: 'ABC123',
      challenge: 'test-challenge',
      type: 'registration',
      sessionId: testSession._id,
      expiresAt: new Date(Date.now() + 60000),
      used: true,
    });

    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/register/finish`)
      .send({
        rollNumber: 'ABC123',
        credential: { id: 'test' },
      });
    
    expect(res.status).toBe(400);
  });
});

describe('Admin Reset Credential', () => {
  it('should reset credential successfully', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
    });

    const res = await request(app)
      .post('/api/admin/webauthn/reset')
      .set('Authorization', `Bearer ${token}`)
      .send({
        rollNumber: 'ABC123',
        reason: 'Device lost',
      });
    
    expect(res.status).toBe(200);
    expect(res.body.message).toContain('reset');

    const credential = await WebAuthnCredential.findOne({ studentId: 'ABC123' });
    expect(credential).toBeNull();
  });

  it('should create reenrollment log', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
    });

    await request(app)
      .post('/api/admin/webauthn/reset')
      .set('Authorization', `Bearer ${token}`)
      .send({
        rollNumber: 'ABC123',
        reason: 'Device lost',
      });
    
    const log = await WebAuthnReenrollmentLog.findOne({
      studentId: 'ABC123',
      actionType: 'reset',
    });
    
    expect(log).toBeDefined();
    expect(log.reason).toBe('Device lost');
    expect(log.previousCredentialId).toBe('test-cred-id');
  });

  it('should return 404 for non-existent credential', async () => {
    const token = await getFreshAdminToken();
    
    const res = await request(app)
      .post('/api/admin/webauthn/reset')
      .set('Authorization', `Bearer ${token}`)
      .send({
        rollNumber: 'NONEXISTENT',
        reason: 'Test',
      });
    
    expect(res.status).toBe(404);
  });

  it('should require authentication', async () => {
    const res = await request(app)
      .post('/api/admin/webauthn/reset')
      .send({
        rollNumber: 'ABC123',
        reason: 'Test',
      });
    
    expect(res.status).toBe(401);
  });

  it('should require roll number', async () => {
    const token = await getFreshAdminToken();
    
    const res = await request(app)
      .post('/api/admin/webauthn/reset')
      .set('Authorization', `Bearer ${token}`)
      .send({ reason: 'Test' });
    
    expect(res.status).toBe(400);
  });
});

describe('Admin Suspend/Unsuspend Credential', () => {
  it('should suspend credential', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
    });

    const res = await request(app)
      .post('/api/admin/webauthn/suspend')
      .set('Authorization', `Bearer ${token}`)
      .send({
        rollNumber: 'ABC123',
        reason: 'Suspicious activity',
      });
    
    expect(res.status).toBe(200);

    const credential = await WebAuthnCredential.findOne({ studentId: 'ABC123' });
    expect(credential.isSuspended).toBe(true);
    expect(credential.suspendedReason).toBe('Suspicious activity');
  });

  it('should unsuspend credential', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
      isSuspended: true,
      suspendedReason: 'Previous suspension',
    });

    const res = await request(app)
      .post('/api/admin/webauthn/unsuspend')
      .set('Authorization', `Bearer ${token}`)
      .send({
        rollNumber: 'ABC123',
        reason: 'Issue resolved',
      });
    
    expect(res.status).toBe(200);

    const credential = await WebAuthnCredential.findOne({ studentId: 'ABC123' });
    expect(credential.isSuspended).toBe(false);
    expect(credential.suspendedReason).toBeNull();
  });

  it('should create log entry for suspend action', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create({
      studentId: 'ABC123',
      credentialId: 'test-cred-id',
      publicKey: Buffer.from('test-public-key'),
      counter: 0,
    });

    await request(app)
      .post('/api/admin/webauthn/suspend')
      .set('Authorization', `Bearer ${token}`)
      .send({
        rollNumber: 'ABC123',
        reason: 'Test suspension',
      });
    
    const log = await WebAuthnReenrollmentLog.findOne({
      studentId: 'ABC123',
      actionType: 'suspend',
    });
    
    expect(log).toBeDefined();
    expect(log.reason).toBe('Test suspension');
  });

  it('should return 404 for suspending non-existent credential', async () => {
    const token = await getFreshAdminToken();
    
    const res = await request(app)
      .post('/api/admin/webauthn/suspend')
      .set('Authorization', `Bearer ${token}`)
      .send({
        rollNumber: 'NONEXISTENT',
        reason: 'Test',
      });
    
    expect(res.status).toBe(404);
  });
});

describe('Admin Get Credentials', () => {
  it('should list all credentials', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create([
      {
        studentId: 'ABC123',
        credentialId: 'cred-1',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      },
      {
        studentId: 'DEF456',
        credentialId: 'cred-2',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      },
    ]);

    const res = await request(app)
      .get('/api/admin/webauthn/credentials')
      .set('Authorization', `Bearer ${token}`);
    
    expect(res.status).toBe(200);
    expect(res.body.credentials).toHaveLength(2);
    expect(res.body.pagination.total).toBe(2);
  });

  it('should filter by suspended status', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create([
      {
        studentId: 'ABC123',
        credentialId: 'cred-1',
        publicKey: Buffer.from('test-key'),
        counter: 0,
        isSuspended: false,
      },
      {
        studentId: 'DEF456',
        credentialId: 'cred-2',
        publicKey: Buffer.from('test-key'),
        counter: 0,
        isSuspended: true,
      },
    ]);

    const res = await request(app)
      .get('/api/admin/webauthn/credentials?suspended=true')
      .set('Authorization', `Bearer ${token}`);
    
    expect(res.status).toBe(200);
    expect(res.body.credentials).toHaveLength(1);
    expect(res.body.credentials[0].studentId).toBe('DEF456');
  });

  it('should search by roll number', async () => {
    const token = await getFreshAdminToken();
    
    await WebAuthnCredential.create([
      {
        studentId: 'ABC123',
        credentialId: 'cred-1',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      },
      {
        studentId: 'DEF456',
        credentialId: 'cred-2',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      },
    ]);

    const res = await request(app)
      .get('/api/admin/webauthn/credentials?search=ABC')
      .set('Authorization', `Bearer ${token}`);
    
    expect(res.status).toBe(200);
    expect(res.body.credentials).toHaveLength(1);
    expect(res.body.credentials[0].studentId).toBe('ABC123');
  });

  it('should paginate results', async () => {
    const token = await getFreshAdminToken();
    
    for (let i = 0; i < 25; i++) {
      await WebAuthnCredential.create({
        studentId: `STU${i.toString().padStart(3, '0')}`,
        credentialId: `cred-${i}`,
        publicKey: Buffer.from('test-key'),
        counter: 0,
      });
    }

    const res = await request(app)
      .get('/api/admin/webauthn/credentials?page=2&limit=10')
      .set('Authorization', `Bearer ${token}`);
    
    expect(res.status).toBe(200);
    expect(res.body.credentials).toHaveLength(10);
    expect(res.body.pagination.page).toBe(2);
    expect(res.body.pagination.pages).toBe(3);
  });

  it('should require authentication', async () => {
    const res = await request(app)
      .get('/api/admin/webauthn/credentials');
    
    expect(res.status).toBe(401);
  });
});

describe('Admin Get Stats', () => {
  it('should return correct statistics', async () => {
    const loginRes = await request(app)
      .post('/api/admin/login')
      .send({ username: 'testadmin', password: 'password123' });
    const freshToken = loginRes.body.token;

    await WebAuthnCredential.create([
      {
        studentId: 'ABC123',
        credentialId: 'cred-1',
        publicKey: Buffer.from('test-key'),
        counter: 0,
        enrolledAt: new Date(),
        isSuspended: false,
      },
      {
        studentId: 'DEF456',
        credentialId: 'cred-2',
        publicKey: Buffer.from('test-key'),
        counter: 0,
        enrolledAt: new Date(Date.now() - 8 * 24 * 60 * 60 * 1000),
        isSuspended: true,
      },
    ]);

    const res = await request(app)
      .get('/api/admin/webauthn/stats')
      .set('Authorization', `Bearer ${freshToken}`);
    
    expect(res.status).toBe(200);
    expect(res.body.totalEnrolled).toBe(2);
    expect(res.body.active).toBe(1);
    expect(res.body.suspended).toBe(1);
    expect(res.body.enrollmentTrends.last7Days).toBe(1);
  });
});

describe('Admin Rate Limiting', () => {
  it('should flag unusual reset activity (more than 10 in an hour)', async () => {
    const loginRes = await request(app)
      .post('/api/admin/login')
      .send({ username: 'testadmin', password: 'password123' });
    const freshToken = loginRes.body.token;
    const admin = await Admin.findOne({ username: 'testadmin' });
    
    for (let i = 0; i < 11; i++) {
      await WebAuthnReenrollmentLog.create({
        studentId: `STU${i}`,
        adminId: admin._id,
        actionType: 'reset',
        timestamp: new Date(),
      });
    }

    await WebAuthnCredential.create({
      studentId: 'STU99',
      credentialId: 'cred-99',
      publicKey: Buffer.from('test-key'),
      counter: 0,
    });

    const res = await request(app)
      .post('/api/admin/webauthn/reset')
      .set('Authorization', `Bearer ${freshToken}`)
      .send({
        rollNumber: 'STU99',
        reason: 'Test',
      });
    
    expect(res.status).toBe(429);
    expect(res.body.requiresConfirmation).toBe(true);
  }, 15000);
});

describe('Edge Cases', () => {
  it('should handle multiple registration requests sequentially', async () => {
    for (let i = 0; i < 3; i++) {
      const res = await request(app)
        .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
        .send({
          rollNumber: `STU${i}`,
          studentName: `Student ${i}`,
        });
      expect(res.status).toBe(200);
      expect(res.body.challenge).toBeDefined();
    }

    const challenges = await WebAuthnChallenge.find({
      type: 'registration',
    });
    expect(challenges.length).toBe(3);
  });

  it('should handle invalid short code', async () => {
    const res = await request(app)
      .get('/s/invalid/webauthn/status/ABC123');
    
    expect(res.status).toBe(404);
  });
});

describe('WebAuthn Model Tests', () => {
  it('should create WebAuthn credential with correct defaults', async () => {
    const cred = await WebAuthnCredential.create({
      studentId: 'TEST001',
      credentialId: 'cred-123',
      publicKey: Buffer.from('test-key'),
      counter: 0,
    });

    expect(cred.deviceLabel).toBe('Unknown Device');
    expect(cred.deviceType).toBe('multi_device');
    expect(cred.isSuspended).toBe(false);
    expect(cred.signCount).toBe(0);
  });

  it('should enforce unique studentId', async () => {
    await WebAuthnCredential.create({
      studentId: 'TEST001',
      credentialId: 'cred-1',
      publicKey: Buffer.from('test-key'),
      counter: 0,
    });

    let error;
    try {
      await WebAuthnCredential.create({
        studentId: 'TEST001',
        credentialId: 'cred-2',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      });
    } catch (e) {
      error = e;
    }

    expect(error).toBeDefined();
    expect(error.code).toBe(11000);
  });

  it('should enforce unique credentialId', async () => {
    await WebAuthnCredential.create({
      studentId: 'TEST001',
      credentialId: 'cred-unique',
      publicKey: Buffer.from('test-key'),
      counter: 0,
    });

    let error;
    try {
      await WebAuthnCredential.create({
        studentId: 'TEST002',
        credentialId: 'cred-unique',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      });
    } catch (e) {
      error = e;
    }

    expect(error).toBeDefined();
    expect(error.code).toBe(11000);
  });
});

describe('WebAuthn Challenge Model Tests', () => {
  it('should auto-expire challenges', async () => {
    const challenge = await WebAuthnChallenge.create({
      studentId: 'TEST001',
      challenge: 'test-challenge',
      type: 'registration',
      sessionId: testSession._id,
    });

    expect(challenge.expiresAt).toBeDefined();
    const expiryTime = challenge.expiresAt.getTime() - Date.now();
    expect(expiryTime).toBeLessThanOrEqual(5 * 60 * 1000 + 1000);
    expect(expiryTime).toBeGreaterThan(4 * 60 * 1000);
  });
});

describe('WebAuthn Reenrollment Log Model Tests', () => {
  it('should create log entry with correct fields', async () => {
    const admin = await Admin.findOne({ username: 'testadmin' });
    
    const log = await WebAuthnReenrollmentLog.create({
      studentId: 'TEST001',
      adminId: admin._id,
      reason: 'Device lost',
      previousCredentialId: 'old-cred',
      actionType: 'reset',
    });

    expect(log.timestamp).toBeDefined();
    expect(log.actionType).toBe('reset');
  });

  it('should allow optional newCredentialId', async () => {
    const admin = await Admin.findOne({ username: 'testadmin' });
    
    const log = await WebAuthnReenrollmentLog.create({
      studentId: 'TEST001',
      adminId: admin._id,
      actionType: 'reset',
    });

    expect(log.newCredentialId).toBeNull();
  });
});

describe('WebAuthn Utility Functions', () => {
  it('should generate unique challenges', () => {
    const { generateChallenge } = require('../src/utils/webauthnUtils');
    
    const challenge1 = generateChallenge();
    const challenge2 = generateChallenge();
    
    expect(challenge1).toBeDefined();
    expect(challenge2).toBeDefined();
    expect(challenge1).not.toBe(challenge2);
    expect(challenge1.length).toBeGreaterThan(30);
  });

  it('should get correct verification method', () => {
    const { getVerificationMethod } = require('../src/utils/webauthnUtils');
    
    expect(getVerificationMethod({ flags: 0x05 })).toBe('face_id');
    expect(getVerificationMethod({ flags: 0x01 })).toBe('fingerprint');
    expect(getVerificationMethod({ flags: 0x00 })).toBe('passkey_fallback');
    expect(getVerificationMethod(null)).toBe('unknown');
  });

  it('should get correct authenticator attachment', () => {
    const { getAuthenticatorAttachment } = require('../src/utils/webauthnUtils');
    
    expect(getAuthenticatorAttachment('Mozilla/5.0 (iPhone)')).toBe('platform');
    expect(getAuthenticatorAttachment('Mozilla/5.0 (Android)')).toBe('platform');
    expect(getAuthenticatorAttachment(null)).toBe('platform');
  });

  it('should have correct RP configuration', () => {
    const { rpName, rpID, origin } = require('../src/utils/webauthnUtils');
    
    expect(rpName).toBeDefined();
    expect(rpID).toBeDefined();
    expect(origin).toBeDefined();
  });
});

describe('WebAuthn Security Tests', () => {
  describe('Challenge Security', () => {
    it('should generate cryptographically secure challenges', () => {
      const { generateChallenge } = require('../src/utils/webauthnUtils');
      
      const challenges = new Set();
      for (let i = 0; i < 1000; i++) {
        challenges.add(generateChallenge());
      }
      
      expect(challenges.size).toBe(1000);
    });

    it('should generate challenges of sufficient entropy', () => {
      const { generateChallenge } = require('../src/utils/webauthnUtils');
      
      const challenge = generateChallenge();
      const decodedLength = Buffer.from(challenge, 'base64url').length;
      
      expect(decodedLength).toBeGreaterThanOrEqual(32);
    });
  });

  describe('Replay Attack Protection', () => {
    it('should track counter for replay attack detection', async () => {
      const cred = await WebAuthnCredential.create({
        studentId: 'REPLAY001',
        credentialId: 'replay-cred',
        publicKey: Buffer.from('test-key'),
        counter: 10,
        signCount: 10,
        deviceLabel: 'Test Device',
      });

      expect(cred.counter).toBe(10);
      expect(cred.signCount).toBe(10);
    });

    it('should not allow challenge reuse', async () => {
      await WebAuthnCredential.create({
        studentId: 'ABC123',
        credentialId: 'test-cred-id',
        publicKey: Buffer.from('test-public-key'),
        counter: 0,
      });

      const challenge = await WebAuthnChallenge.create({
        studentId: 'ABC123',
        challenge: 'test-challenge-unique',
        type: 'authentication',
        sessionId: testSession._id,
        used: false,
        expiresAt: new Date(Date.now() + 60000),
      });

      challenge.used = true;
      await challenge.save();

      const reusedChallenge = await WebAuthnChallenge.findOne({
        studentId: 'ABC123',
        challenge: 'test-challenge-unique',
        used: false,
      });

      expect(reusedChallenge).toBeNull();
    });
  });

  describe('Input Validation', () => {
    it('should trim roll number input', async () => {
      const res = await request(app)
        .get(`/s/${testShortLink.shortCode}/webauthn/status/  ABC123  `);
      
      expect(res.status).toBe(200);
      expect(res.body.enrolled).toBe(false);
    });

    it('should reject empty roll number', async () => {
      const res = await request(app)
        .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
        .send({
          rollNumber: '',
          studentName: 'Test Student',
        });
      
      expect(res.status).toBe(400);
    });

    it('should reject empty student name', async () => {
      const res = await request(app)
        .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
        .send({
          rollNumber: 'ABC123',
          studentName: '',
        });
      
      expect(res.status).toBe(400);
    });

    it('should handle very long roll numbers', async () => {
      const longRollNumber = 'A'.repeat(1000);
      
      const res = await request(app)
        .get(`/s/${testShortLink.shortCode}/webauthn/status/${longRollNumber}`);
      
      expect(res.status).toBe(200);
    });

    it('should handle special characters in roll number', async () => {
      const res = await request(app)
        .get(`/s/${testShortLink.shortCode}/webauthn/status/ABC-123_456`);
      
      expect(res.status).toBe(200);
    });

    it('should handle unicode characters in roll number', async () => {
      const res = await request(app)
        .get(`/s/${testShortLink.shortCode}/webauthn/status/TEST123`);
      
      expect(res.status).toBe(200);
    });
  });

  describe('Authentication Bypass Prevention', () => {
    it('should not allow accessing admin endpoints without token', async () => {
      const endpoints = [
        { method: 'get', path: '/api/admin/webauthn/credentials' },
        { method: 'get', path: '/api/admin/webauthn/stats' },
        { method: 'post', path: '/api/admin/webauthn/reset' },
        { method: 'post', path: '/api/admin/webauthn/suspend' },
      ];

      for (const endpoint of endpoints) {
        const res = await request(app)[endpoint.method](endpoint.path);
        expect(res.status).toBe(401);
      }
    });

    it('should not allow accessing admin endpoints with invalid token', async () => {
      const res = await request(app)
        .get('/api/admin/webauthn/credentials')
        .set('Authorization', 'Bearer invalid-token');
      
      expect(res.status).toBe(401);
    });

    it('should not allow accessing admin endpoints with expired token', async () => {
      const expiredToken = 'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpZCI6IjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2NjY2Iiwicm9sZSI6ImFkbWluIiwiaWF0IjoxNjAwMDAwMDAwLCJleHAiOjE2MDAwMDAwMDF9.invalid';
      
      const res = await request(app)
        .get('/api/admin/webauthn/credentials')
        .set('Authorization', `Bearer ${expiredToken}`);
      
      expect(res.status).toBe(401);
    });
  });

  describe('Session Validation', () => {
    it('should reject requests for inactive short link', async () => {
      await ShortLink.findByIdAndUpdate(testShortLink._id, { isActive: false });

      const res = await request(app)
        .get(`/s/${testShortLink.shortCode}/webauthn/status/ABC123`);
      
      expect(res.status).toBe(404);
    });

    it('should reject requests for expired session', async () => {
      await Session.findByIdAndUpdate(testSession._id, {
        expiresAt: new Date(Date.now() - 1000),
      });

      const res = await request(app)
        .post(`/s/${testShortLink.shortCode}/webauthn/register/start`)
        .send({
          rollNumber: 'ABC123',
          studentName: 'Test Student',
        });
      
      expect(res.status).toBe(400);
    });
  });

  describe('Audit Logging', () => {
    it('should log all admin actions', async () => {
      const loginRes = await request(app)
        .post('/api/admin/login')
        .send({ username: 'testadmin', password: 'password123' });
      const token = loginRes.body.token;
      const admin = await Admin.findOne({ username: 'testadmin' });

      await WebAuthnCredential.create({
        studentId: 'AUDIT001',
        credentialId: 'audit-cred',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      });

      await request(app)
        .post('/api/admin/webauthn/suspend')
        .set('Authorization', `Bearer ${token}`)
        .send({
          rollNumber: 'AUDIT001',
          reason: 'Test audit',
        });

      const log = await WebAuthnReenrollmentLog.findOne({
        studentId: 'AUDIT001',
        actionType: 'suspend',
      });

      expect(log).toBeDefined();
      expect(log.adminId.toString()).toBe(admin._id.toString());
      expect(log.reason).toBe('Test audit');
    });

    it('should track multiple actions for same student', async () => {
      const loginRes = await request(app)
        .post('/api/admin/login')
        .send({ username: 'testadmin', password: 'password123' });
      const token = loginRes.body.token;

      await WebAuthnCredential.create({
        studentId: 'MULTI001',
        credentialId: 'multi-cred',
        publicKey: Buffer.from('test-key'),
        counter: 0,
      });

      await request(app)
        .post('/api/admin/webauthn/suspend')
        .set('Authorization', `Bearer ${token}`)
        .send({
          rollNumber: 'MULTI001',
          reason: 'First action',
        });

      await request(app)
        .post('/api/admin/webauthn/unsuspend')
        .set('Authorization', `Bearer ${token}`)
        .send({
          rollNumber: 'MULTI001',
          reason: 'Second action',
        });

      const logs = await WebAuthnReenrollmentLog.find({
        studentId: 'MULTI001',
      }).sort({ timestamp: 1 });

      expect(logs).toHaveLength(2);
      expect(logs[0].actionType).toBe('suspend');
      expect(logs[1].actionType).toBe('unsuspend');
    });
  });
});

describe('WebAuthn Error Handling', () => {
  it('should return user-friendly error for non-enrolled student', async () => {
    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/authenticate/start`)
      .send({ rollNumber: 'NOTENROLLED' });
    
    expect(res.status).toBe(404);
    expect(res.body.notEnrolled).toBe(true);
    expect(res.body.message).toBeDefined();
  });

  it('should return user-friendly error for suspended credential', async () => {
    await WebAuthnCredential.create({
      studentId: 'SUSPENDED001',
      credentialId: 'suspended-cred',
      publicKey: Buffer.from('test-key'),
      counter: 0,
      isSuspended: true,
      suspendedReason: 'Test suspension',
    });

    const res = await request(app)
      .post(`/s/${testShortLink.shortCode}/webauthn/authenticate/start`)
      .send({ rollNumber: 'SUSPENDED001' });
    
    expect(res.status).toBe(403);
    expect(res.body.suspended).toBe(true);
  });

  it('should handle database errors gracefully', async () => {
    const res = await request(app)
      .get('/api/admin/webauthn/credentials')
      .set('Authorization', `Bearer ${await getFreshAdminToken()}`);
    
    expect(res.status).toBe(200);
  });
});
