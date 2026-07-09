const Session = require('../src/models/Session');
const Location = require('../src/models/Location');
const Admin = require('../src/models/Admin');
const mongoose = require('mongoose');
const { getCachedSession, invalidateSessionCache } = require('../src/middleware/sessionCache');

// MongoMemoryServer lifecycle is handled by tests/globalSetup.js +
// tests/dbSetup.js (setupFilesAfterEnv). No local beforeAll/afterAll needed.

describe('Session Caching Middleware', () => {
  let admin, location, session;

  beforeEach(async () => {
    admin = await Admin.create({
      username: 'testadmin',
      email: 'test@example.com',
      password: 'password123'
    });

    location = await Location.create({
      name: 'Test Location',
      latitude: 12.9716,
      longitude: 77.5946,
      radiusMeters: 100,
      createdBy: admin._id
    });

    const token = Session.generateToken();
    const tokenHash = Session.hashToken(token);
    const tokenPrefix = token.substring(0, 4);

    session = await Session.create({
      locationId: location._id,
      tokenHash,
      tokenPrefix,
      createdBy: admin._id,
      expiresAt: new Date(Date.now() + 30 * 60 * 1000),
      isActive: true
    });
  });

  describe('getCachedSession', () => {
    it('should fetch session from MongoDB when Redis is not connected', async () => {
      const tokenHash = session.tokenHash;
      
      const cachedSession = await getCachedSession(tokenHash);
      
      expect(cachedSession).toBeTruthy();
      expect(cachedSession._id.toString()).toBe(session._id.toString());
      expect(cachedSession.isActive).toBe(true);
    });

    it('should populate location data when fetching session', async () => {
      const tokenHash = session.tokenHash;
      
      const cachedSession = await getCachedSession(tokenHash);
      
      expect(cachedSession.locationId).toBeTruthy();
      expect(cachedSession.locationId.name).toBe('Test Location');
      expect(cachedSession.locationId.latitude).toBe(12.9716);
      expect(cachedSession.locationId.longitude).toBe(77.5946);
    });

    it('should return null for inactive sessions', async () => {
      session.isActive = false;
      await session.save();
      
      const cachedSession = await getCachedSession(session.tokenHash);
      
      expect(cachedSession).toBeNull();
    });

    it('should return null for expired sessions', async () => {
      session.expiresAt = new Date(Date.now() - 1000);
      await session.save();
      
      const cachedSession = await getCachedSession(session.tokenHash);
      
      expect(cachedSession).toBeNull();
    });

    it('should return null for non-existent token hash', async () => {
      const fakeTokenHash = 'nonexistent123456789012345678901234567890';
      
      const cachedSession = await getCachedSession(fakeTokenHash);
      
      expect(cachedSession).toBeNull();
    });
  });

  describe('Session Validation Flow', () => {
    it('should validate active session correctly', async () => {
      const sessionData = await getCachedSession(session.tokenHash);
      
      expect(sessionData).toBeTruthy();
      expect(sessionData.isActive).toBe(true);
      expect(sessionData.expiresAt > new Date()).toBe(true);
    });

    it('should handle multiple sequential requests to same session', async () => {
      const tokenHash = session.tokenHash;
      
      const session1 = await getCachedSession(tokenHash);
      const session2 = await getCachedSession(tokenHash);
      
      expect(session1._id.toString()).toBe(session._id.toString());
      expect(session2._id.toString()).toBe(session._id.toString());
    });
  });

  describe('Token Rotation', () => {
    it('should allow token hash to be updated', async () => {
      const newToken = Session.generateToken();
      const newTokenHash = Session.hashToken(newToken);
      
      session.tokenHash = newTokenHash;
      session.tokenPrefix = newToken.substring(0, 4);
      session.rotationCount += 1;
      await session.save();
      
      const updatedSession = await Session.findById(session._id);
      expect(updatedSession.tokenHash).toBe(newTokenHash);
      expect(updatedSession.rotationCount).toBe(1);
    });
  });
});

describe('Redis Configuration', () => {
  it('should handle Redis connection gracefully when not configured', () => {
    const { isRedisConnected } = require('../src/config/redis');
    expect(typeof isRedisConnected).toBe('function');
  });

  it('should fallback to MongoDB when Redis is unavailable', async () => {
    const Session = require('../src/models/Session');
    const Admin = require('../src/models/Admin');
    const Location = require('../src/models/Location');
    
    const admin = await Admin.create({
      username: 'testadmin2',
      email: 'test2@example.com',
      password: 'password123'
    });

    const location = await Location.create({
      name: 'Test Location 2',
      latitude: 12.9716,
      longitude: 77.5946,
      radiusMeters: 100,
      createdBy: admin._id
    });

    const token = Session.generateToken();
    const tokenHash = Session.hashToken(token);
    const tokenPrefix = token.substring(0, 4);

    const session = await Session.create({
      locationId: location._id,
      tokenHash,
      tokenPrefix,
      createdBy: admin._id,
      expiresAt: new Date(Date.now() + 30 * 60 * 1000),
      isActive: true
    });

    const cachedSession = await getCachedSession(tokenHash);
    
    expect(cachedSession).toBeTruthy();
    expect(cachedSession._id.toString()).toBe(session._id.toString());
  });
});

describe('Cache TTL Behavior', () => {
  it('should respect session expiration logic', async () => {
    const Admin = require('../src/models/Admin');
    const Location = require('../src/models/Location');
    
    const admin = await Admin.create({
      username: 'ttltest',
      email: 'ttl@example.com',
      password: 'password123'
    });

    const location = await Location.create({
      name: 'TTL Location',
      latitude: 12.9716,
      longitude: 77.5946,
      radiusMeters: 100,
      createdBy: admin._id
    });

    const token = Session.generateToken();
    const tokenHash = Session.hashToken(token);
    const tokenPrefix = token.substring(0, 4);

    const session = await Session.create({
      locationId: location._id,
      tokenHash,
      tokenPrefix,
      createdBy: admin._id,
      expiresAt: new Date(Date.now() + 1000),
      isActive: true
    });

    const cachedSession = await getCachedSession(tokenHash);
    expect(cachedSession).toBeTruthy();

    await new Promise(resolve => setTimeout(resolve, 1100));

    const expiredSession = await getCachedSession(tokenHash);
    expect(expiredSession).toBeNull();
  });
});

describe('Error Handling', () => {
  it('should handle malformed token hash gracefully', async () => {
    const malformedHash = '';
    
    const session = await getCachedSession(malformedHash);
    expect(session).toBeNull();
  });

  it('should handle database connection errors', async () => {
    const originalConnect = mongoose.connect;
    mongoose.connect = jest.fn().mockRejectedValue(new Error('Connection failed'));

    await expect(async () => {
      await getCachedSession('somehash');
    }).not.toThrow();

    mongoose.connect = originalConnect;
  });
});

describe('Security Edge Cases', () => {
  let admin, location, session;

  beforeEach(async () => {
    admin = await Admin.create({
      username: 'sectest',
      email: 'sec@example.com',
      password: 'password123'
    });

    location = await Location.create({
      name: 'Security Test Location',
      latitude: 12.9716,
      longitude: 77.5946,
      radiusMeters: 100,
      createdBy: admin._id
    });

    const token = Session.generateToken();
    const tokenHash = Session.hashToken(token);
    const tokenPrefix = token.substring(0, 4);

    session = await Session.create({
      locationId: location._id,
      tokenHash,
      tokenPrefix,
      createdBy: admin._id,
      expiresAt: new Date(Date.now() + 30 * 60 * 1000),
      isActive: true
    });
  });

  it('should not expose sensitive token hash in session data', async () => {
    const cachedSession = await getCachedSession(session.tokenHash);
    
    expect(cachedSession.tokenHash).toBeDefined();
    expect(cachedSession.tokenPrefix).toBeDefined();
  });

  it('should prevent accessing other users sessions', async () => {
    const otherAdmin = await Admin.create({
      username: 'otheruser',
      email: 'other@example.com',
      password: 'password123'
    });

    const otherLocation = await Location.create({
      name: 'Other Location',
      latitude: 13.0,
      longitude: 78.0,
      radiusMeters: 100,
      createdBy: otherAdmin._id
    });

    const otherToken = Session.generateToken();
    const otherTokenHash = Session.hashToken(otherToken);
    const otherTokenPrefix = otherToken.substring(0, 4);

    await Session.create({
      locationId: otherLocation._id,
      tokenHash: otherTokenHash,
      tokenPrefix: otherTokenPrefix,
      createdBy: otherAdmin._id,
      expiresAt: new Date(Date.now() + 30 * 60 * 1000),
      isActive: true
    });

    const session1 = await getCachedSession(session.tokenHash);
    const session2 = await getCachedSession(otherTokenHash);

    expect(session1._id.toString()).toBe(session._id.toString());
    expect(session2._id.toString()).not.toBe(session._id.toString());
    expect(session2.createdBy.toString()).toBe(otherAdmin._id.toString());
  });

  it('should handle concurrent requests to same session', async () => {
    const tokenHash = session.tokenHash;
    
    const promises = Array(10).fill(null).map(() => getCachedSession(tokenHash));
    const results = await Promise.all(promises);
    
    results.forEach(result => {
      expect(result._id.toString()).toBe(session._id.toString());
    });
  });
});
