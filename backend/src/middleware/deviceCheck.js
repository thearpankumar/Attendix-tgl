const Device = require('../models/Device');
const Attendance = require('../models/Attendance');

async function validateDeviceFingerprint(req, res, next) {
  try {
    const { rollNumber, sessionId, deviceFingerprint } = req.body;
    
    if (!deviceFingerprint) {
      req.deviceValidation = {
        valid: true,
        firstSeen: true,
        warning: 'No device fingerprint provided',
      };
      return next();
    }

    const fingerprintHash = Device.hashFingerprint(deviceFingerprint);
    req.deviceValidation = {
      fingerprintHash,
      valid: true,
      firstSeen: false,
      flags: [],
    };

    const existingDevice = await Device.findOne({
      fingerprintHash,
      sessionId,
    });

    if (existingDevice) {
      existingDevice.lastSeenAt = new Date();
      existingDevice.attendanceCount += 1;
      
      if (existingDevice.boundToStudent !== rollNumber.toUpperCase()) {
        existingDevice.addFlag('MULTI_STUDENT_DEVICE', 
          `Device previously used by ${existingDevice.boundToStudent}, now ${rollNumber}`,
          sessionId
        );
        req.deviceValidation.flags.push('MULTI_STUDENT_DEVICE');
        req.deviceValidation.deviceFlag = 'MULTI_STUDENT_DEVICE';
      }
      
      await existingDevice.save();
      req.deviceValidation.device = existingDevice;
    } else {
      const studentExistingDevice = await Device.findOne({
        boundToStudent: rollNumber.toUpperCase(),
        sessionId,
      });

      if (studentExistingDevice && studentExistingDevice.fingerprintHash !== fingerprintHash) {
        req.deviceValidation.flags.push('STUDENT_DEVICE_SWITCHED');
        req.deviceValidation.deviceFlag = 'STUDENT_DEVICE_SWITCHED';
        req.deviceValidation.previousDevice = studentExistingDevice;
      }

      const newDevice = new Device({
        fingerprintHash,
        boundToStudent: rollNumber.toUpperCase(),
        sessionId,
        firstSeenAt: new Date(),
        lastSeenAt: new Date(),
        attendanceCount: 1,
        metadata: {
          userAgent: req.headers['user-agent'],
        },
      });

      if (req.deviceValidation.flags.length > 0) {
        req.deviceValidation.flags.forEach(flag => {
          newDevice.addFlag(flag, 'Detected during attendance submission', sessionId);
        });
      }

      await newDevice.save();
      req.deviceValidation.device = newDevice;
      req.deviceValidation.firstSeen = true;
    }

    next();
  } catch (error) {
    if (process.env.NODE_ENV !== 'test') console.error('Device validation error:', error);
    req.deviceValidation = {
      valid: true,
      warning: 'Device validation failed, allowing submission',
    };
    next();
  }
}

async function checkRapidSubmission(req, res, next) {
  try {
    const { rollNumber, sessionId } = req.body;
    const recentThreshold = new Date(Date.now() - 10 * 1000);

    const recentAttendance = await Attendance.findOne({
      sessionId,
      rollNumber: rollNumber.toUpperCase(),
      capturedAt: { $gte: recentThreshold },
    });

    if (recentAttendance) {
      req.deviceValidation = req.deviceValidation || { valid: true, flags: [] };
      req.deviceValidation.flags.push('RAPID_SUBMISSION');
      req.deviceValidation.deviceFlag = 'RAPID_SUBMISSION';
      req.deviceValidation.rapidSubmissionWarning = 'Attendance already submitted within last 10 seconds';
    }

    next();
  } catch (error) {
    if (process.env.NODE_ENV !== 'test') console.error('Rapid submission check error:', error);
    next();
  }
}

module.exports = {
  validateDeviceFingerprint,
  checkRapidSubmission,
};
