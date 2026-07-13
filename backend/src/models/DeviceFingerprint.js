const mongoose = require('mongoose');

const DeviceFingerprintSchema = new mongoose.Schema({
  fingerprintId: {
    type: String,
    required: true,
    unique: true,
    index: true,
  },
  
  firstSeen: {
    type: Date,
    default: Date.now,
  },
  
  lastSeen: {
    type: Date,
    default: Date.now,
  },
  
  verificationFailures: {
    type: Number,
    default: 0,
  },
  
  spoofingAttempts: {
    type: Number,
    default: 0,
  },
  
  lastSpoofingReason: {
    type: String,
  },
  
  inconsistencies: [{
    type: String,
  }],
  
  claimedDeviceTypes: [{
    type: String,
  }],
  
  userAgentsSeen: [{
    ua: String,
    firstSeen: Date,
    lastSeen: Date,
  }],
  
  sessions: [{
    sessionId: {
      type: mongoose.Schema.Types.ObjectId,
      ref: 'Session',
    },
    rollNumber: String,
    timestamp: Date,
    wasSuccessful: Boolean,
  }],
  
  isTrusted: {
    type: Boolean,
    default: false,
  },
  
  isBlocked: {
    type: Boolean,
    default: false,
  },
  
  blockReason: {
    type: String,
  },
  
  lastMetrics: {
    maxTouchPoints: Number,
    hardwareConcurrency: Number,
    deviceMemory: Number,
    webglRenderer: String,
    screenWidth: Number,
    screenHeight: Number,
    platform: String,
  },
  
}, {
  timestamps: true,
});

DeviceFingerprintSchema.methods.recordVerificationFailure = function(reason) {
  this.verificationFailures += 1;
  this.lastSeen = new Date();
  
  if (reason) {
    this.lastSpoofingReason = reason;
    this.spoofingAttempts += 1;
    
    if (!this.isBlocked && this.spoofingAttempts >= 5) {
      this.isBlocked = true;
      this.blockReason = `Blocked after ${this.spoofingAttempts} spoofing attempts`;
    }
  }
  
  return this.save();
};

DeviceFingerprintSchema.methods.recordSuccessfulVerification = function(sessionId, rollNumber) {
  this.lastSeen = new Date();
  
  if (this.verificationFailures > 0) {
    this.verificationFailures = Math.max(0, this.verificationFailures - 1);
  }
  
  if (this.sessions.length >= 50) {
    this.sessions = this.sessions.slice(-49);
  }
  
  this.sessions.push({
    sessionId,
    rollNumber,
    timestamp: new Date(),
    wasSuccessful: true,
  });
  
  const successfulCount = this.sessions.filter(s => s.wasSuccessful).length;
  if (successfulCount >= 3 && this.spoofingAttempts === 0) {
    this.isTrusted = true;
  }
  
  return this.save();
};

DeviceFingerprintSchema.methods.addUserAgent = function(userAgent) {
  const existing = this.userAgentsSeen.find(u => u.ua === userAgent);
  if (existing) {
    existing.lastSeen = new Date();
  } else {
    if (this.userAgentsSeen.length >= 20) {
      this.userAgentsSeen = this.userAgentsSeen.slice(-19);
    }
    this.userAgentsSeen.push({
      ua: userAgent,
      firstSeen: new Date(),
      lastSeen: new Date(),
    });
  }
  return this.save();
};

DeviceFingerprintSchema.methods.addClaimedDeviceType = function(deviceType) {
  if (!this.claimedDeviceTypes.includes(deviceType)) {
    this.claimedDeviceTypes.push(deviceType);
  }
  return this.save();
};

DeviceFingerprintSchema.statics.findOrCreate = async function(fingerprintId) {
  let device = await this.findOne({ fingerprintId });
  if (!device) {
    device = await this.create({ fingerprintId });
  }
  return device;
};

DeviceFingerprintSchema.statics.findByRollNumber = async function(rollNumber) {
  const devices = await this.find({
    'sessions.rollNumber': rollNumber,
  });
  return devices;
};

DeviceFingerprintSchema.statics.getBlockedDevices = async function() {
  return this.find({ isBlocked: true });
};

DeviceFingerprintSchema.statics.getSuspiciousDevices = async function(threshold = 3) {
  return this.find({
    $or: [
      { spoofingAttempts: { $gte: threshold } },
      { verificationFailures: { $gte: threshold * 2 } },
    ],
    isBlocked: false,
  });
};

module.exports = mongoose.model('DeviceFingerprint', DeviceFingerprintSchema);
