const express = require('express');
const router = express.Router();
const ShortLink = require('../models/ShortLink');
const Session = require('../models/Session');
const Attendance = require('../models/Attendance');
const Location = require('../models/Location');
const Device = require('../models/Device');
const { studentLimiter } = require('../middleware/rateLimiter');
const { generateTOTPWithTimestamp, validateTOTPCode, validateQRToken } = require('../utils/totpUtils');
const { getStorageProvider } = require('../storage');
const { calculateDistance } = require('../utils/geoUtils');
const svgCaptcha = require('svg-captcha');
const crypto = require('crypto');
const config = require('../config');

const signCaptchaText = (text, timestamp) => {
  return crypto
    .createHmac('sha256', config.jwtSecret || 'fallback-secret')
    .update(`${text.toLowerCase()}:${timestamp}`)
    .digest('hex');
};

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

router.post('/:shortCode/submit', studentLimiter, async (req, res) => {
  try {
    const { shortCode } = req.params;
    const {
      studentName,
      rollNumber,
      photo,
      latitude,
      longitude,
      directUpload = false,
      publicId = null,
      faceDetected,
      captchaAnswer,
      captchaId,
      totpCode,
      deviceFingerprint,
    } = req.body;

    // Verify Captcha (bypass in testing)
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
        return res.status(400).json({ message: 'Captcha expired. Please refresh and try again.' });
      }

      const expectedSignature = signCaptchaText(captchaAnswer, timestamp);
      if (expectedSignature !== signature) {
        return res.status(400).json({ message: 'Incorrect captcha. Please try again.' });
      }
    }

    const shortLink = await ShortLink.findOne({ 
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');

    if (!shortLink) {
      return res.status(404).json({ message: 'Invalid short link' });
    }

    if (!shortLink.sessionId) {
      return res.status(400).json({ message: 'No session attached to this link' });
    }

    const session = await Session.findById(shortLink.sessionId._id).populate('locationId');

    if (!session || !session.isActive) {
      return res.status(400).json({ message: 'Session is not active' });
    }

    if (session.expiresAt && new Date() > session.expiresAt) {
      return res.status(400).json({ message: 'Session has expired' });
    }

    if (!session.locationId) {
      return res.status(404).json({ message: 'Location not found for this session' });
    }

    // TOTP validation
    let totpValid = null;
    if (session.totpEnabled) {
      if (!totpCode) {
        return res.status(400).json({ 
          message: 'This session requires a time-based code. Please scan the QR code or enter the current code.',
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
          message: 'Invalid or expired code. Please get the current code and try again.',
          totpRequired: true,
          totpValid: false,
        });
      }
    }

    // Check for existing attendance
    const existingAttendance = await Attendance.findOne({
      sessionId: session._id,
      rollNumber: rollNumber.toUpperCase(),
    });

    if (existingAttendance) {
      return res.status(400).json({
        message: 'Attendance already submitted for this roll number',
      });
    }

    // Device fingerprint validation
    let deviceFingerprintHash = null;
    let deviceFirstSeen = false;
    let deviceFlag = null;

    if (deviceFingerprint) {
      deviceFingerprintHash = Device.hashFingerprint(deviceFingerprint);
      
      const existingDevice = await Device.findOne({
        fingerprintHash: deviceFingerprintHash,
        sessionId: session._id,
      });

      if (existingDevice) {
        existingDevice.lastSeenAt = new Date();
        existingDevice.attendanceCount += 1;
        
        if (existingDevice.boundToStudent !== rollNumber.toUpperCase()) {
          existingDevice.addFlag('MULTI_STUDENT_DEVICE', 
            `Device previously used by ${existingDevice.boundToStudent}, now ${rollNumber}`,
            session._id
          );
          deviceFlag = 'MULTI_STUDENT_DEVICE';
        }
        
        await existingDevice.save();
      } else {
        const studentExistingDevice = await Device.findOne({
          boundToStudent: rollNumber.toUpperCase(),
          sessionId: session._id,
        });

        if (studentExistingDevice && studentExistingDevice.fingerprintHash !== deviceFingerprintHash) {
          deviceFlag = 'STUDENT_DEVICE_SWITCHED';
        }

        const newDevice = new Device({
          fingerprintHash: deviceFingerprintHash,
          boundToStudent: rollNumber.toUpperCase(),
          sessionId: session._id,
          firstSeenAt: new Date(),
          lastSeenAt: new Date(),
          attendanceCount: 1,
          metadata: {
            userAgent: req.get('User-Agent'),
          },
        });

        if (deviceFlag) {
          newDevice.addFlag(deviceFlag, `Detected during attendance submission`, session._id);
        }

        await newDevice.save();
        deviceFirstSeen = true;
      }
    }

    // Handle photo upload
    let photoUrl = '';
    let photoPublicId = '';

    const storage = getStorageProvider();

    if (directUpload && publicId) {
      photoUrl = storage.getFileUrl(publicId);
      photoPublicId = publicId;
    } else if (photo) {
      try {
        const uploadResult = await storage.upload(photo, {
          folder: 'attendance-photos',
          key: `${session._id}_${rollNumber}_${Date.now()}`,
        });
        photoUrl = uploadResult.url;
        photoPublicId = uploadResult.publicId;
      } catch (uploadError) {
        console.error('Photo upload error details:', uploadError);
        return res.status(400).json({
          message: 'Failed to upload photo',
          error: uploadError.message,
        });
      }
    } else {
      return res.status(400).json({
        message: 'Photo is required',
      });
    }

    // Calculate distance
    const distance = calculateDistance(
      latitude,
      longitude,
      session.locationId.latitude,
      session.locationId.longitude
    );

    const isWithinGeofence = distance <= session.locationId.radiusMeters;

    // Get network info
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

    // Create attendance record
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
      faceDetected: faceDetected !== undefined ? faceDetected : true,
      deviceFingerprint: deviceFingerprint,
      deviceFingerprintHash: deviceFingerprintHash,
      deviceFirstSeen: deviceFirstSeen,
      deviceFlag: deviceFlag,
      totpCode: totpCode || null,
      totpValid: totpValid,
      flagReviewed: false,
    });

    res.status(201).json({
      message: deviceFlag 
        ? 'Attendance submitted successfully. Note: Device flagged for review.'
        : 'Attendance submitted successfully',
      attendance: {
        _id: attendance._id,
        studentName: attendance.studentName,
        rollNumber: attendance.rollNumber,
        distanceFromLocation: attendance.distanceFromLocation,
        verified: attendance.verified,
        capturedAt: attendance.capturedAt,
        deviceFirstSeen: attendance.deviceFirstSeen,
        deviceFlag: attendance.deviceFlag,
      },
      deviceWarning: deviceFlag ? [deviceFlag] : null,
    });
  } catch (error) {
    if (error.code === 11000) {
      return res.status(400).json({
        message: 'Attendance already submitted for this roll number',
      });
    }
    console.error('Submit attendance error:', error);
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.get('/:shortCode/info', studentLimiter, async (req, res) => {
  try {
    const { shortCode } = req.params;
    
    const shortLink = await ShortLink.findOne({ 
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');

    if (!shortLink) {
      return res.status(404).json({ message: 'Short link not found' });
    }

    if (!shortLink.sessionId) {
      return res.status(400).json({ message: 'No session attached to this link' });
    }

    const session = shortLink.sessionId;
    
    if (!session.isActive || (session.expiresAt && new Date() > session.expiresAt)) {
      return res.status(400).json({ message: 'Session is not active' });
    }

    const totpData = generateTOTPWithTimestamp(
      session.totpSecret,
      session._id.toString(),
      session.totpWindowSeconds
    );

    res.json({
      sessionId: session._id,
      totpCode: totpData.code,
      expiresAt: totpData.expiresAt,
      windowSeconds: totpData.windowSeconds,
      sessionActive: session.isActive,
      sessionExpiresAt: session.expiresAt,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.get('/:shortCode/session', studentLimiter, async (req, res) => {
  try {
    const { shortCode } = req.params;
    
    const shortLink = await ShortLink.findOne({ 
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');

    if (!shortLink) {
      return res.status(404).json({ message: 'Short link not found' });
    }

    if (!shortLink.sessionId) {
      return res.status(400).json({ message: 'No session attached to this link' });
    }

    const session = shortLink.sessionId;
    const location = await Location.findById(session.locationId);
    
    if (!session.isActive || (session.expiresAt && new Date() > session.expiresAt)) {
      return res.status(400).json({ message: 'Session is not active' });
    }

    // QR anti-sharing validation: if ?qrt= is present, verify the 4-second rotating token
    const { qrt } = req.query;
    if (qrt) {
      const qrtResult = validateQRToken(shortCode, session.totpSecret, qrt);
      if (!qrtResult.valid) {
        return res.status(403).json({
          qrExpired: true,
          message: 'QR code has expired. Please scan the current QR code shown on screen.',
        });
      }
    }

    res.json({
      valid: true,
      session: {
        _id: session._id,
        locationName: location ? location.name : 'Unknown',
        expiresAt: session.expiresAt,
        totpEnabled: session.totpEnabled,
      },
      shortLink: shortCode,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
});

router.get('/:shortCode', studentLimiter, async (req, res) => {
  try {
    const { shortCode } = req.params;
    
    const shortLink = await ShortLink.findOne({ 
      shortCode: shortCode.toLowerCase(),
      isActive: true,
    }).populate('sessionId');

    if (!shortLink) {
      return res.status(404).send(`
        <!DOCTYPE html>
        <html>
        <head>
          <title>Link Not Found</title>
          <style>
            body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
            h1 { color: #e74c3c; }
          </style>
        </head>
        <body>
          <h1>Invalid Link</h1>
          <p>This short link does not exist or has been deactivated.</p>
          <p>Please contact your instructor for a valid attendance link.</p>
        </body>
        </html>
      `);
    }

    if (shortLink.expiresAt && new Date() > shortLink.expiresAt) {
      return res.status(410).send(`
        <!DOCTYPE html>
        <html>
        <head>
          <title>Link Expired</title>
          <style>
            body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
            h1 { color: #e67e22; }
          </style>
        </head>
        <body>
          <h1>Link Expired</h1>
          <p>This attendance link has expired.</p>
          <p>Please contact your instructor for a new link.</p>
        </body>
        </html>
      `);
    }

    if (!shortLink.sessionId) {
      return res.status(400).send(`
        <!DOCTYPE html>
        <html>
        <head>
          <title>No Session Attached</title>
          <style>
            body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
            h1 { color: #f39c12; }
          </style>
        </head>
        <body>
          <h1>Link Not Configured</h1>
          <p>This short link has not been attached to an attendance session yet.</p>
          <p>Please contact your instructor.</p>
        </body>
        </html>
      `);
    }

    const session = shortLink.sessionId;

    if (!session.isActive) {
      return res.status(400).send(`
        <!DOCTYPE html>
        <html>
        <head>
          <title>Session Inactive</title>
          <style>
            body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
            h1 { color: #95a5a6; }
          </style>
        </head>
        <body>
          <h1>Session Inactive</h1>
          <p>This attendance session is no longer active.</p>
          <p>Please contact your instructor.</p>
        </body>
        </html>
      `);
    }

    if (session.expiresAt && new Date() > session.expiresAt) {
      return res.status(410).send(`
        <!DOCTYPE html>
        <html>
        <head>
          <title>Session Expired</title>
          <style>
            body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
            h1 { color: #e67e22; }
          </style>
        </head>
        <body>
          <h1>Session Expired</h1>
          <p>This attendance session has ended.</p>
          <p>Please contact your instructor if you need to mark attendance.</p>
        </body>
        </html>
      `);
    }

    shortLink.clickCount += 1;
    shortLink.lastClickedAt = new Date();
    await shortLink.save();

    const studentAppUrl = `/attend/${shortCode}`;

    res.redirect(studentAppUrl);
  } catch (error) {
    console.error('Short link redirect error:', error);
    res.status(500).send(`
      <!DOCTYPE html>
      <html>
      <head>
        <title>Error</title>
        <style>
          body { font-family: Arial, sans-serif; text-align: center; padding: 50px; }
          h1 { color: #e74c3c; }
        </style>
      </head>
      <body>
        <h1>Error</h1>
        <p>Something went wrong. Please try again later.</p>
      </body>
      </html>
    `);
  }
});

module.exports = router;
