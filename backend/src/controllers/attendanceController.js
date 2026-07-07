const Session = require('../models/Session');
const Attendance = require('../models/Attendance');
const { getStorageProvider } = require('../storage');
const { calculateDistance } = require('../utils/geoUtils');
const { validateTOTPCode } = require('../utils/totpUtils');
const { getCachedSession } = require('../middleware/sessionCache');
const svgCaptcha = require('svg-captcha');
const crypto = require('crypto');
const config = require('../config');

const signCaptchaText = (text, timestamp) => {
  return crypto
    .createHmac('sha256', config.jwtSecret || 'fallback-secret')
    .update(`${text.toLowerCase()}:${timestamp}`)
    .digest('hex');
};

const validateToken = async (req, res) => {
  try {
    const { token } = req.params;
    const tokenHash = Session.hashToken(token);

    const session = await getCachedSession(tokenHash);

    if (!session) {
      return res.status(404).json({
        valid: false,
        message: 'Invalid or expired attendance link',
      });
    }

    if (!session.locationId) {
      return res.status(404).json({
        valid: false,
        message: 'Location not found for this session',
      });
    }

    res.json({
      valid: true,
      session: {
        _id: session._id,
        locationName: session.locationId.name,
        expiresAt: session.expiresAt,
      },
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getUploadUrl = async (req, res) => {
  try {
    const { token } = req.params;
    const tokenHash = Session.hashToken(token);

    const session = await getCachedSession(tokenHash);

    if (!session) {
      return res.status(404).json({
        message: 'Invalid or expired attendance link',
      });
    }

    const storage = getStorageProvider();
    const key = `${session._id}_${Date.now()}`;
    
    const uploadInfo = await storage.getUploadUrl(key, 'image/jpeg');

    res.json({
      uploadUrl: uploadInfo.uploadUrl,
      publicId: uploadInfo.publicId,
      method: uploadInfo.method,
      headers: uploadInfo.headers || {},
      contentType: uploadInfo.contentType || 'image/jpeg',
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const submitAttendance = async (req, res) => {
  try {
    const { token } = req.params;
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

      // Check expiry: 5 minutes limit
      const FIVE_MINUTES_MS = 5 * 60 * 1000;
      if (isNaN(timestamp) || Date.now() - timestamp > FIVE_MINUTES_MS) {
        return res.status(400).json({ message: 'Captcha expired. Please refresh and try again.' });
      }

      // Recompute signature
      const expectedSignature = signCaptchaText(captchaAnswer, timestamp);
      if (expectedSignature !== signature) {
        return res.status(400).json({ message: 'Incorrect captcha. Please try again.' });
      }
    }

    const tokenHash = Session.hashToken(token);

    const session = await Session.findOne({
      tokenHash,
      isActive: true,
      expiresAt: { $gt: new Date() },
    }).populate('locationId');

    if (!session) {
      return res.status(404).json({
        message: 'Invalid or expired attendance link',
      });
    }

    if (!session.locationId) {
      return res.status(404).json({
        message: 'Location not found for this session',
      });
    }

    // TOTP validation for sessions with TOTP enabled
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
        1 // 1 window tolerance
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

    // Device fingerprint validation
    const deviceValidation = req.deviceValidation || { valid: true, firstSeen: false };
    const deviceFingerprintHash = deviceValidation.fingerprintHash || null;

    const existingAttendance = await Attendance.findOne({
      sessionId: session._id,
      rollNumber: rollNumber.toUpperCase(),
    });

    if (existingAttendance) {
      return res.status(400).json({
        message: 'Attendance already submitted for this roll number',
      });
    }

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
        if (process.env.NODE_ENV !== 'test') console.error('Photo upload error details:', uploadError);
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
    if (ip && ip !== '::1' && ip !== '127.0.0.1' && !ip.startsWith('::ffff:') && !ip.startsWith('192.168.') && !ip.startsWith('10.') && !/^172\.(1[6-9]|2[0-9]|3[0-1])\./.test(ip)) {
      try {
        const controller = new AbortController();
        const timeoutId = setTimeout(() => controller.abort(), 2000);
        
        const ipApiUrl = process.env.IP_API_URL || 'http://ip-api.com/json/';
        const ipRes = await fetch(`${ipApiUrl}${ip}?fields=status,message,isp,org`, {
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
      deviceFirstSeen: deviceValidation.firstSeen,
      deviceFlag: deviceValidation.deviceFlag || null,
      totpCode: totpCode || null,
      totpValid: totpValid,
      flagReviewed: false,
    });

    const responseMessage = deviceValidation.flags && deviceValidation.flags.length > 0
      ? 'Attendance submitted successfully. Note: Device flagged for review.'
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
        deviceFirstSeen: attendance.deviceFirstSeen,
        deviceFlag: attendance.deviceFlag,
      },
      deviceWarning: deviceValidation.flags && deviceValidation.flags.length > 0 
        ? deviceValidation.flags 
        : null,
    });
  } catch (error) {
    if (error.code === 11000) {
      return res.status(400).json({
        message: 'Attendance already submitted for this roll number',
      });
    }
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const checkAttendanceStatus = async (req, res) => {
  try {
    const { token } = req.params;
    const { rollNumber } = req.query;

    if (!rollNumber) {
      return res.status(400).json({ message: 'Roll number required' });
    }

    const tokenHash = Session.hashToken(token);

    const session = await Session.findOne({
      tokenHash,
      isActive: true,
      expiresAt: { $gt: new Date() },
    });

    if (!session) {
      return res.status(404).json({
        message: 'Invalid or expired attendance link',
      });
    }

    const existingAttendance = await Attendance.findOne({
      sessionId: session._id,
      rollNumber: rollNumber.toUpperCase(),
    });

    res.json({
      alreadySubmitted: !!existingAttendance,
      attendance: existingAttendance
        ? {
            studentName: existingAttendance.studentName,
            rollNumber: existingAttendance.rollNumber,
            verified: existingAttendance.verified,
            capturedAt: existingAttendance.capturedAt,
          }
        : null,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getStorageInfo = async (req, res) => {
  try {
    const storage = getStorageProvider();
    res.json({
      provider: storage.getName(),
      supportsDirectUpload: typeof storage.getUploadUrl === 'function',
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getCaptcha = async (req, res) => {
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
};

module.exports = {
  validateToken,
  getUploadUrl,
  submitAttendance,
  checkAttendanceStatus,
  getStorageInfo,
  getCaptcha,
};
