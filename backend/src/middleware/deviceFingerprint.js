const DeviceFingerprint = require('../models/DeviceFingerprint');

async function checkDeviceFingerprint(req, res, next) {
  if (process.env.NODE_ENV === 'test') {
    return next();
  }

  const { deviceFingerprint } = req.body;
  
  if (!deviceFingerprint) {
    return res.status(400).json({
      success: false,
      message: 'Device fingerprint is required',
    });
  }
  
  try {
    const device = await DeviceFingerprint.findOrCreate(deviceFingerprint);
    
    if (device.isBlocked) {
      return res.status(403).json({
        success: false,
        message: 'This device has been blocked due to suspicious activity',
        reason: device.blockReason,
      });
    }
    
    res.locals.deviceRecord = device;
    
    next();
  } catch (error) {
    console.error('Device fingerprint check error:', error);
    next();
  }
}

async function recordDeviceSuccess(req, res, next) {
  if (!res.locals.deviceRecord) {
    return next();
  }
  
  const device = res.locals.deviceRecord;
  const sessionId = res.locals.session?._id || req.body.sessionId;
  const rollNumber = req.body.rollNumber;
  
  try {
    await device.recordSuccessfulVerification(sessionId, rollNumber);
    await device.addUserAgent(req.headers['user-agent'] || 'unknown');
    await device.addClaimedDeviceType(req.body.devBypassWebauthn ? 'dev-bypass' : 'normal');
  } catch (error) {
    console.error('Record device success error:', error);
  }
  
  next();
}

async function recordDeviceFailure(fingerprintId, reason) {
  if (!fingerprintId || process.env.NODE_ENV === 'test') return;
  
  try {
    const device = await DeviceFingerprint.findOrCreate(fingerprintId);
    await device.recordVerificationFailure(reason);
  } catch (error) {
    console.error('Record device failure error:', error);
  }
}

async function getDeviceTrustScore(fingerprintId) {
  if (!fingerprintId) return 0;
  
  try {
    const device = await DeviceFingerprint.findOne({ fingerprintId });
    if (!device) return 50;
    
    if (device.isBlocked) return 0;
    if (device.isTrusted) return 100;
    
    const successRate = device.sessions.filter(s => s.wasSuccessful).length;
    const failRate = device.verificationFailures;
    
    const baseScore = 50;
    const successBonus = Math.min(successRate * 5, 40);
    const failPenalty = Math.min(failRate * 10, 45);
    const spoofingPenalty = device.spoofingAttempts * 15;
    
    const trustScore = Math.max(0, Math.min(100, baseScore + successBonus - failPenalty - spoofingPenalty));
    return trustScore;
  } catch (error) {
    console.error('Get device trust score error:', error);
    return 50;
  }
}

async function flagSuspiciousDevice(fingerprintId, inconsistencies) {
  if (!fingerprintId || process.env.NODE_ENV === 'test') return;
  
  try {
    const device = await DeviceFingerprint.findOrCreate(fingerprintId);
    
    device.inconsistencies = [...new Set([...device.inconsistencies, ...inconsistencies])];
    await device.recordVerificationFailure('Spoofing detected: ' + inconsistencies.join(', '));
  } catch (error) {
    console.error('Flag suspicious device error:', error);
  }
}

module.exports = {
  checkDeviceFingerprint,
  recordDeviceSuccess,
  recordDeviceFailure,
  getDeviceTrustScore,
  flagSuspiciousDevice,
};
