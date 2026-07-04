const express = require('express');
const router = express.Router();
const ShortLink = require('../models/ShortLink');
const Session = require('../models/Session');
const Attendance = require('../models/Attendance');
const WebAuthnCredential = require('../models/WebAuthnCredential');
const WebAuthnChallenge = require('../models/WebAuthnChallenge');
const Device = require('../models/Device');
const { studentLimiter } = require('../middleware/rateLimiter');
const { getStorageProvider } = require('../storage');
const { calculateDistance } = require('../utils/geoUtils');
const { validateTOTPCode } = require('../utils/totpUtils');
const svgCaptcha = require('svg-captcha');
const crypto = require('crypto');
const config = require('../config');
const {
  generateChallenge,
  createRegistrationOptions,
  verifyRegistration,
  createAuthenticationOptions,
  verifyAuthentication,
  getVerificationMethod,
  getAuthenticatorAttachment,
} = require('../utils/webauthnUtils');

const signCaptchaText = (text, timestamp) => {
  return crypto
    .createHmac('sha256', config.jwtSecret || 'fallback-secret')
    .update(`${text.toLowerCase()}:${timestamp}`)
    .digest('hex');
};

router.get('/:shortCode/webauthn/status/:rollNumber', async (req, res) => {
  try {
    const { shortCode, rollNumber } = req.params;
    
    const shortLink = await ShortLink.findOne({
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');
    
    if (!shortLink || !shortLink.sessionId) {
      return res.status(404).json({ message: 'Invalid session' });
    }
    
    const session = shortLink.sessionId;
    if (!session.isActive || (session.expiresAt && new Date() > session.expiresAt)) {
      return res.status(400).json({ message: 'Session expired' });
    }
    
    const credential = await WebAuthnCredential.findOne({
      studentId: rollNumber.toUpperCase(),
    });
    
    const existingAttendance = await Attendance.findOne({
      sessionId: session._id,
      rollNumber: rollNumber.toUpperCase(),
    });
    
    if (existingAttendance) {
      return res.json({
        enrolled: !!credential,
        suspended: credential?.isSuspended || false,
        alreadySubmitted: true,
        message: 'Attendance already submitted',
      });
    }
    
    res.json({
      enrolled: !!credential,
      suspended: credential?.isSuspended || false,
      alreadySubmitted: false,
      studentName: credential?.deviceLabel || null,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.post('/:shortCode/verify-gatekeeper', async (req, res) => {
  try {
    const { shortCode } = req.params;
    const { rollNumber, totpCode } = req.body;
    
    if (!rollNumber) {
      return res.status(400).json({ message: 'Roll number is required' });
    }
    
    const shortLink = await ShortLink.findOne({
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');
    
    if (!shortLink || !shortLink.sessionId) {
      return res.status(404).json({ message: 'Invalid session' });
    }
    
    const session = shortLink.sessionId;
    if (!session.isActive || (session.expiresAt && new Date() > session.expiresAt)) {
      return res.status(400).json({ message: 'Session expired' });
    }

    // Verify TOTP early
    if (session.totpEnabled) {
      if (!totpCode) {
        return res.status(400).json({ 
          message: 'This session requires a time-based code. Please enter the current code.',
          totpRequired: true,
        });
      }
      
      const totpResult = validateTOTPCode(
        totpCode,
        session.totpSecret,
        session._id.toString(),
        session.totpWindowSeconds,
        1
      );
      
      if (!totpResult.valid) {
        return res.status(400).json({ 
          message: 'Invalid or expired code. Please get the current code from the projector.',
          totpRequired: true,
        });
      }
    }
    
    // Check WebAuthn status
    const credential = await WebAuthnCredential.findOne({
      studentId: rollNumber.toUpperCase(),
    });
    
    const existingAttendance = await Attendance.findOne({
      sessionId: session._id,
      rollNumber: rollNumber.toUpperCase(),
    });
    
    if (existingAttendance) {
      return res.json({
        valid: true,
        enrolled: !!credential,
        suspended: credential?.isSuspended || false,
        alreadySubmitted: true,
        message: 'Attendance already submitted',
      });
    }
    
    res.json({
      valid: true,
      enrolled: !!credential,
      suspended: credential?.isSuspended || false,
      alreadySubmitted: false,
      studentName: credential?.deviceLabel || null,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.post('/:shortCode/webauthn/register/start', async (req, res) => {
  try {
    const { shortCode } = req.params;
    const { rollNumber, studentName } = req.body;
    
    if (!rollNumber || !studentName) {
      return res.status(400).json({ message: 'Roll number and name required' });
    }
    
    const shortLink = await ShortLink.findOne({
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');
    
    if (!shortLink || !shortLink.sessionId) {
      return res.status(404).json({ message: 'Invalid session' });
    }
    
    const session = shortLink.sessionId;
    if (!session.isActive || (session.expiresAt && new Date() > session.expiresAt)) {
      return res.status(400).json({ message: 'Session expired' });
    }
    
    const existingCredential = await WebAuthnCredential.findOne({
      studentId: rollNumber.toUpperCase(),
    });
    
    if (existingCredential) {
      return res.status(400).json({
        message: 'Device already enrolled. Contact admin to re-enroll on a new device.',
        alreadyEnrolled: true,
      });
    }
    
    const challenge = generateChallenge();
    
    await WebAuthnChallenge.create({
      studentId: rollNumber.toUpperCase(),
      challenge,
      type: 'registration',
      sessionId: session._id,
      shortCode: shortCode.toLowerCase(),
      studentName,
      expiresAt: new Date(Date.now() + 5 * 60 * 1000),
    });
    
    const options = await createRegistrationOptions(
      rollNumber.toUpperCase(),
      rollNumber.toUpperCase(),
      studentName,
      []
    );
    
    options.challenge = challenge;
    
    res.json(options);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.post('/:shortCode/webauthn/register/finish', async (req, res) => {
  try {
    const { rollNumber, credential } = req.body;
    
    if (!rollNumber || !credential) {
      return res.status(400).json({ message: 'Roll number and credential required' });
    }
    
    const storedChallenge = await WebAuthnChallenge.findOne({
      studentId: rollNumber.toUpperCase(),
      type: 'registration',
      used: false,
      expiresAt: { $gt: new Date() },
    }).sort({ createdAt: -1 });
    
    if (!storedChallenge) {
      return res.status(400).json({ message: 'No valid registration challenge found' });
    }
    
    const verification = await verifyRegistration(credential, storedChallenge.challenge);
    
    if (!verification.verified) {
      return res.status(400).json({ message: 'Registration verification failed' });
    }
    
    const { registrationInfo } = verification;
    
    const existingCredential = await WebAuthnCredential.findOne({
      studentId: rollNumber.toUpperCase(),
    });
    
    if (existingCredential) {
      return res.status(400).json({ message: 'Device already enrolled' });
    }
    
    const newCredential = await WebAuthnCredential.create({
      studentId: rollNumber.toUpperCase(),
      credentialId: registrationInfo.credential.id,
      publicKey: Buffer.from(registrationInfo.credential.publicKey),
      counter: registrationInfo.credential.counter,
      deviceLabel: storedChallenge.studentName,
      deviceType: registrationInfo.credentialDeviceType,
      transports: registrationInfo.credential.transports || [],
      enrolledAt: new Date(),
      enrolledIpAddress: req.ip,
      enrolledUserAgent: req.get('User-Agent'),
      aaguid: registrationInfo.aaguid || null,
    });
    
    storedChallenge.used = true;
    await storedChallenge.save();
    
    res.json({
      verified: true,
      credentialId: newCredential.credentialId,
      message: 'Device enrolled successfully',
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.post('/:shortCode/webauthn/authenticate/start', async (req, res) => {
  try {
    const { shortCode } = req.params;
    const { rollNumber } = req.body;
    
    if (!rollNumber) {
      return res.status(400).json({ message: 'Roll number required' });
    }
    
    const shortLink = await ShortLink.findOne({
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');
    
    if (!shortLink || !shortLink.sessionId) {
      return res.status(404).json({ message: 'Invalid session' });
    }
    
    const session = shortLink.sessionId;
    if (!session.isActive || (session.expiresAt && new Date() > session.expiresAt)) {
      return res.status(400).json({ message: 'Session expired' });
    }
    
    const credential = await WebAuthnCredential.findOne({
      studentId: rollNumber.toUpperCase(),
    });
    
    if (!credential) {
      return res.status(404).json({
        message: 'No credential found. Please enroll your device first.',
        notEnrolled: true,
      });
    }
    
    if (credential.isSuspended) {
      return res.status(403).json({
        message: 'Your credential has been suspended. Please contact admin.',
        suspended: true,
      });
    }
    
    const challenge = generateChallenge();
    
    await WebAuthnChallenge.create({
      studentId: rollNumber.toUpperCase(),
      challenge,
      type: 'authentication',
      sessionId: session._id,
      shortCode: shortCode.toLowerCase(),
      expiresAt: new Date(Date.now() + 5 * 60 * 1000),
    });
    
    const options = await createAuthenticationOptions([{
      credentialId: credential.credentialId,
      transports: credential.transports || [],
    }]);
    
    options.challenge = challenge;
    
    res.json(options);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.post('/:shortCode/webauthn/authenticate/finish', studentLimiter, async (req, res) => {
  try {
    const { shortCode } = req.params;
    const {
      rollNumber,
      credential,
      studentName,
      photo,
      latitude,
      longitude,
      captchaAnswer,
      captchaId,
      totpCode,
      deviceFingerprint,
    } = req.body;
    
    if (!rollNumber || !credential) {
      return res.status(400).json({ message: 'Roll number and credential required' });
    }
    
    const storedChallenge = await WebAuthnChallenge.findOne({
      studentId: rollNumber.toUpperCase(),
      type: 'authentication',
      used: false,
      expiresAt: { $gt: new Date() },
    }).sort({ createdAt: -1 });
    
    if (!storedChallenge) {
      return res.status(400).json({ message: 'No valid authentication challenge found' });
    }
    
    const shortLink = await ShortLink.findOne({
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');
    
    if (!shortLink || !shortLink.sessionId) {
      return res.status(404).json({ message: 'Invalid session' });
    }
    
    const session = await Session.findById(shortLink.sessionId._id).populate('locationId');
    
    if (!session || !session.isActive) {
      return res.status(400).json({ message: 'Session is not active' });
    }
    
    if (session.expiresAt && new Date() > session.expiresAt) {
      return res.status(400).json({ message: 'Session has expired' });
    }
    
    const storedCredential = await WebAuthnCredential.findOne({
      studentId: rollNumber.toUpperCase(),
    });
    
    if (!storedCredential) {
      return res.status(404).json({ message: 'Credential not found' });
    }
    
    if (storedCredential.isSuspended) {
      return res.status(403).json({ message: 'Credential suspended' });
    }
    
    const verification = await verifyAuthentication(credential, storedChallenge.challenge, {
      credentialId: storedCredential.credentialId,
      publicKey: storedCredential.publicKey,
      counter: storedCredential.counter,
      transports: storedCredential.transports || [],
    });
    
    if (!verification.verified) {
      return res.status(400).json({ message: 'Authentication verification failed' });
    }
    
    const { authenticationInfo } = verification;
    const newCounter = authenticationInfo.newCounter;
    
    let replayAttack = false;
    if (newCounter <= storedCredential.counter) {
      replayAttack = true;
    }
    
    storedCredential.counter = newCounter;
    storedCredential.lastUsedAt = new Date();
    storedCredential.lastSessionId = session._id;
    storedCredential.signCount += 1;
    await storedCredential.save();
    
    storedChallenge.used = true;
    await storedChallenge.save();
    
    if (process.env.NODE_ENV !== 'test') {
      if (!captchaAnswer || !captchaId) {
        return res.status(400).json({ message: 'Captcha verification inputs missing' });
      }
      
      const parts = captchaId.split('.');
      if (parts.length !== 2) {
        return res.status(400).json({ message: 'Invalid Captcha ID format' });
      }
      
      const [timestampStr, signature] = parts;
      const timestamp = parseInt(timestampStr, 10);
      
      const FIVE_MINUTES_MS = 5 * 60 * 1000;
      if (isNaN(timestamp) || Date.now() - timestamp > FIVE_MINUTES_MS) {
        return res.status(400).json({ message: 'Captcha expired' });
      }
      
      const expectedSignature = signCaptchaText(captchaAnswer, timestamp);
      if (expectedSignature !== signature) {
        return res.status(400).json({ message: 'Incorrect captcha' });
      }
    }
    
    let totpValid = null;
    if (session.totpEnabled) {
      if (!totpCode) {
        return res.status(400).json({
          message: 'This session requires a time-based code.',
          totpRequired: true,
        });
      }
      
      const totpResult = validateTOTPCode(
        totpCode,
        session.totpSecret,
        session._id.toString(),
        session.totpWindowSeconds,
        1
      );
      
      totpValid = totpResult.valid;
      
      if (!totpValid) {
        return res.status(400).json({
          message: 'Invalid or expired code.',
          totpRequired: true,
        });
      }
    }
    
    const existingAttendance = await Attendance.findOne({
      sessionId: session._id,
      rollNumber: rollNumber.toUpperCase(),
    });
    
    if (existingAttendance) {
      return res.status(400).json({ message: 'Attendance already submitted' });
    }
    
    let photoUrl = '';
    let photoPublicId = '';
    
    const storage = getStorageProvider();
    
    if (photo) {
      try {
        const uploadResult = await storage.upload(photo, {
          folder: 'attendance-photos',
          key: `${session._id}_${rollNumber}_${Date.now()}`,
        });
        photoUrl = uploadResult.url;
        photoPublicId = uploadResult.publicId;
      } catch (uploadError) {
        return res.status(400).json({
          message: 'Failed to upload photo',
          error: uploadError.message,
        });
      }
    } else {
      return res.status(400).json({ message: 'Photo is required' });
    }
    
    const distance = calculateDistance(
      latitude,
      longitude,
      session.locationId.latitude,
      session.locationId.longitude
    );
    
    const isWithinGeofence = distance <= session.locationId.radiusMeters;
    
    let networkProvider = 'Unknown';
    let networkOrg = 'Unknown';
    
    const ip = req.ip;
    if (ip && ip !== '::1' && ip !== '127.0.0.1' && !ip.startsWith('192.168.') && !ip.startsWith('10.') && !ip.startsWith('172.')) {
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 2000);
        
        const ipRes = await fetch(`http://ip-api.com/json/${ip}?fields=status,message,isp,org`, {
          signal: controller.signal
        });
        clearTimeout(timeoutId);
        
        if (ipRes.ok) {
          const ipData = await ipRes.json();
          if (ipData.status === 'success') {
            networkProvider = ipData.isp || 'Unknown';
            networkOrg = ipData.org || 'Unknown';
          }
        }
      } catch (err) {
        console.warn('Failed to fetch ISP information:', err.message);
      }
    }
    
    let deviceFingerprintHash = null;
    if (deviceFingerprint) {
      deviceFingerprintHash = Device.hashFingerprint(deviceFingerprint);
    }
    
    const verificationMethod = getVerificationMethod(credential.response?.authenticatorData);
    const authenticatorAttachment = getAuthenticatorAttachment(req.get('User-Agent'));
    
    let deviceFlag = null;
    if (replayAttack) {
      deviceFlag = 'WEBAUTHN_REPLAY_ATTACK';
    }
    
    const attendance = await Attendance.create({
      sessionId: session._id,
      studentName,
      rollNumber: rollNumber.toUpperCase(),
      photoUrl,
      photoPublicId,
      studentLatitude: latitude,
      studentLongitude: longitude,
      distanceFromLocation: Math.round(distance),
      ipAddress: ip,
      userAgent: req.get('User-Agent'),
      networkProvider,
      networkOrg,
      verified: isWithinGeofence,
      faceDetected: true,
      deviceFingerprint,
      deviceFingerprintHash,
      deviceFirstSeen: false,
      totpCode: totpCode || null,
      totpValid,
      deviceFlag,
      webauthnCredentialId: storedCredential.credentialId,
      webauthnVerified: true,
      webauthnDeviceType: verificationMethod,
      webauthnAuthenticatorAttachment: authenticatorAttachment,
      webauthnCounter: newCounter,
      webauthnReplayAttack: replayAttack,
      flagReviewed: false,
    });
    
    const responseMessage = replayAttack
      ? 'Attendance submitted. Note: Device flagged for security review.'
      : 'Attendance submitted successfully';
    
    res.status(201).json({
      message: responseMessage,
      attendance: {
        _id: attendance._id,
        studentName: attendance.studentName,
        rollNumber: attendance.rollNumber,
        distanceFromLocation: attendance.distanceFromLocation,
        verified: attendance.verified,
        capturedAt: attendance.capturedAt,
        webauthnVerified: attendance.webauthnVerified,
        webauthnDeviceType: attendance.webauthnDeviceType,
      },
      replayAttack,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.get('/:shortCode/captcha', async (req, res) => {
  try {
    const captcha = svgCaptcha.create({
      size: 4,
      noise: 2,
      color: true,
      background: '#f1f1f1',
    });
    
    const timestamp = Date.now();
    const signature = signCaptchaText(captcha.text, timestamp);
    const captchaId = `${timestamp}.${signature}`;
    
    res.setHeader('Cache-Control', 'no-store, no-cache, must-revalidate, proxy-revalidate');
    res.setHeader('Pragma', 'no-cache');
    res.setHeader('Expires', '0');
    
    res.json({
      captchaSvg: captcha.data,
      captchaId: captchaId,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error generating captcha', error: error.message });
  }
});

module.exports = router;
