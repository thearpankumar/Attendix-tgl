const mongoose = require('mongoose');
const DeviceFingerprint = require('../src/models/DeviceFingerprint');

describe('DeviceFingerprint Model', () => {
  beforeAll(async () => {
    const uri = process.env.MONGO_TEST_URI || 'mongodb://localhost:27017/test';
    await mongoose.connect(uri);
  });

  afterAll(async () => {
    await mongoose.connection.dropDatabase();
    await mongoose.connection.close();
  });

  beforeEach(async () => {
    await DeviceFingerprint.deleteMany({});
  });

  describe('findOrCreate', () => {
    it('creates a new device fingerprint if not exists', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-123');
      
      expect(device).toBeDefined();
      expect(device.fingerprintId).toBe('fingerprint-123');
      expect(device.verificationFailures).toBe(0);
      expect(device.spoofingAttempts).toBe(0);
      expect(device.isBlocked).toBe(false);
    });

    it('returns existing device fingerprint', async () => {
      await DeviceFingerprint.create({ fingerprintId: 'fingerprint-456' });
      
      const device = await DeviceFingerprint.findOrCreate('fingerprint-456');
      
      expect(device.fingerprintId).toBe('fingerprint-456');
    });
  });

  describe('recordVerificationFailure', () => {
    it('increments verification failures', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-fail-test');
      await device.recordVerificationFailure('Test failure');
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-fail-test' });
      expect(updated.verificationFailures).toBe(1);
      expect(updated.lastSpoofingReason).toBe('Test failure');
      expect(updated.spoofingAttempts).toBe(1);
    });

    it('increments spoofing attempts only when reason provided', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-fail-no-reason');
      await device.recordVerificationFailure(null);
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-fail-no-reason' });
      expect(updated.verificationFailures).toBe(1);
      expect(updated.spoofingAttempts).toBe(0);
    });

    it('blocks device after 5 spoofing attempts', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-block-test');
      
      for (let i = 0; i < 5; i++) {
        await device.recordVerificationFailure('Spoofing attempt ' + (i + 1));
      }
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-block-test' });
      expect(updated.isBlocked).toBe(true);
      expect(updated.blockReason).toContain('Blocked after 5 spoofing attempts');
    });
  });

  describe('recordSuccessfulVerification', () => {
    it('adds session to device history', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-success-test');
      const sessionId = new mongoose.Types.ObjectId();
      
      await device.recordSuccessfulVerification(sessionId, 'ROLL001');
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-success-test' });
      expect(updated.sessions).toHaveLength(1);
      expect(updated.sessions[0].rollNumber).toBe('ROLL001');
      expect(updated.sessions[0].wasSuccessful).toBe(true);
    });

    it('reduces verification failures on success', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-recovery-test');
      await device.recordVerificationFailure(null);
      await device.recordVerificationFailure(null);
      
      const sessionId = new mongoose.Types.ObjectId();
      await device.recordSuccessfulVerification(sessionId, 'ROLL002');
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-recovery-test' });
      expect(updated.verificationFailures).toBe(1);
    });

    it('marks device as trusted after 3 successful sessions with no spoofing', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-trust-test');
      
      for (let i = 0; i < 3; i++) {
        const sessionId = new mongoose.Types.ObjectId();
        await device.recordSuccessfulVerification(sessionId, 'ROLL' + i);
      }
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-trust-test' });
      expect(updated.isTrusted).toBe(true);
    });

    it('does not mark as trusted if spoofing attempts exist', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-no-trust-test');
      await device.recordVerificationFailure('Spoofing attempt');
      
      for (let i = 0; i < 3; i++) {
        const sessionId = new mongoose.Types.ObjectId();
        await device.recordSuccessfulVerification(sessionId, 'ROLL' + i);
      }
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-no-trust-test' });
      expect(updated.isTrusted).toBe(false);
    });
  });

  describe('addUserAgent', () => {
    it('adds new user agent', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-ua-test');
      await device.addUserAgent('Mozilla/5.0 Test UA');
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-ua-test' });
      expect(updated.userAgentsSeen).toHaveLength(1);
      expect(updated.userAgentsSeen[0].ua).toBe('Mozilla/5.0 Test UA');
    });

    it('updates lastSeen for existing user agent', async () => {
      const device = await DeviceFingerprint.findOrCreate('fingerprint-ua-update-test');
      await device.addUserAgent('Mozilla/5.0 Test UA');
      
      await new Promise(resolve => setTimeout(resolve, 10));
      
      await device.addUserAgent('Mozilla/5.0 Test UA');
      
      const updated = await DeviceFingerprint.findOne({ fingerprintId: 'fingerprint-ua-update-test' });
      expect(updated.userAgentsSeen).toHaveLength(1);
      expect(updated.userAgentsSeen[0].lastSeen).not.toEqual(updated.userAgentsSeen[0].firstSeen);
    });
  });

  describe('Static Methods', () => {
    it('finds suspicious devices', async () => {
      const device = await DeviceFingerprint.create({ 
        fingerprintId: 'suspicious-1',
        spoofingAttempts: 4,
      });
      
      const device2 = await DeviceFingerprint.create({ 
        fingerprintId: 'suspicious-2',
        verificationFailures: 10,
      });
      
      const suspiciousDevices = await DeviceFingerprint.getSuspiciousDevices(3);
      
      expect(suspiciousDevices.length).toBe(2);
    });

    it('finds blocked devices', async () => {
      await DeviceFingerprint.create({ 
        fingerprintId: 'blocked-1',
        isBlocked: true,
        blockReason: 'Abuse detected',
      });
      
      const blockedDevices = await DeviceFingerprint.getBlockedDevices();
      
      expect(blockedDevices.length).toBe(1);
      expect(blockedDevices[0].fingerprintId).toBe('blocked-1');
    });

    it('finds devices by roll number', async () => {
      const sessionId = new mongoose.Types.ObjectId();
      const device = await DeviceFingerprint.create({ 
        fingerprintId: 'roll-test-device',
        sessions: [{
          sessionId,
          rollNumber: 'TESTROLL',
          timestamp: new Date(),
          wasSuccessful: true,
        }],
      });
      
      const devices = await DeviceFingerprint.findByRollNumber('TESTROLL');
      
      expect(devices.length).toBe(1);
    });
  });
});
