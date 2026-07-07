const Session = require('../models/Session');
const Location = require('../models/Location');
const Attendance = require('../models/Attendance');
const Admin = require('../models/Admin');
const ShortLink = require('../models/ShortLink');
const Device = require('../models/Device');
const { getStorageProvider } = require('../storage');
const { generateTOTPWithTimestamp, generateQRToken } = require('../utils/totpUtils');
const { invalidateSessionCache } = require('../middleware/sessionCache');

const createSession = async (req, res) => {
  try {
    const { locationId, durationMinutes, description } = req.body;

    const location = await Location.findOne({
      _id: locationId,
      createdBy: req.admin._id,
    });

    if (!location) {
      return res.status(404).json({ message: 'Location not found' });
    }

    const duration = durationMinutes || 30;
    const expiresAt = new Date(Date.now() + duration * 60 * 1000);

    const token = Session.generateToken();
    const tokenHash = Session.hashToken(token);
    const tokenPrefix = token.substring(0, 4);

    const session = await Session.create({
      locationId,
      tokenHash,
      tokenPrefix,
      description,
      createdBy: req.admin._id,
      expiresAt,
      totpEnabled: false,
    });

    res.status(201).json({
      _id: session._id,
      locationId: session.locationId,
      locationName: location.name,
      token,
      tokenPrefix: session.tokenPrefix,
      description: session.description,
      expiresAt: session.expiresAt,
      isActive: session.isActive,
      totpEnabled: session.totpEnabled,
      createdAt: session.createdAt,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getSessions = async (req, res) => {
  try {
    const sessions = await Session.find({ createdBy: req.admin._id })
      .populate('locationId', 'name latitude longitude radiusMeters')
      .select('-totpSecret')
      .sort({ createdAt: -1 });

    const sessionsWithStats = await Promise.all(
      sessions.map(async (session) => {
        const attendanceCount = await Attendance.countDocuments({
          sessionId: session._id,
        });
        const obj = session.toObject();
        delete obj.totpSecret; // Belt-and-suspenders: remove even if select missed it
        return {
          ...obj,
          attendanceCount,
        };
      })
    );

    res.json(sessionsWithStats);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getSessionById = async (req, res) => {
  try {
    const session = await Session.findOne({
      _id: req.params.id,
      createdBy: req.admin._id,
    }).populate('locationId', 'name latitude longitude radiusMeters');

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }

    const attendanceCount = await Attendance.countDocuments({
      sessionId: session._id,
    });

    res.json({
      ...session.toObject(),
      attendanceCount,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const rotateToken = async (req, res) => {
  try {
    const session = await Session.findOne({
      _id: req.params.id,
      createdBy: req.admin._id,
    });

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }

    await invalidateSessionCache(session.tokenHash);

    const newToken = Session.generateToken();
    session.tokenHash = Session.hashToken(newToken);
    session.tokenPrefix = newToken.substring(0, 4);
    session.rotationCount += 1;

    await session.save();

    res.json({
      _id: session._id,
      token: newToken,
      tokenPrefix: session.tokenPrefix,
      rotationCount: session.rotationCount,
      message: 'Token rotated successfully',
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const deactivateSession = async (req, res) => {
  try {
    const session = await Session.findOneAndUpdate(
      { _id: req.params.id, createdBy: req.admin._id },
      { isActive: false },
      { new: true }
    );

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }

    res.json({ message: 'Session deactivated', session });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getSessionAttendance = async (req, res) => {
  try {
    const session = await Session.findOne({
      _id: req.params.id,
      createdBy: req.admin._id,
    });

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }

    const attendance = await Attendance.find({ sessionId: session._id })
      .sort({ capturedAt: -1 });

    res.json(attendance);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getSessionStats = async (req, res) => {
  try {
    const session = await Session.findOne({
      _id: req.params.id,
      createdBy: req.admin._id,
    });

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }

    const totalAttendance = await Attendance.countDocuments({
      sessionId: session._id,
    });
    const verifiedAttendance = await Attendance.countDocuments({
      sessionId: session._id,
      verified: true,
    });

    res.json({
      totalAttendance,
      verifiedAttendance,
      unverifiedAttendance: totalAttendance - verifiedAttendance,
      session: {
        isActive: session.isActive,
        expiresAt: session.expiresAt,
        rotationCount: session.rotationCount,
      },
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const deleteSession = async (req, res) => {
  try {
    // First verify session ownership — so cross-admin deletions get 404, not 400
    const session = await Session.findOne({
      _id: req.params.id,
      createdBy: req.admin._id,
    });

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }

    const { password } = req.body;

    if (!password) {
      return res.status(400).json({ message: 'Password is required to delete a session' });
    }

    // Re-fetch admin WITH the password field (protect middleware strips it with .select('-password'))
    const adminWithPassword = await Admin.findById(req.admin._id);
    if (!adminWithPassword) {
      return res.status(401).json({ message: 'Admin not found' });
    }

    const isMatch = await adminWithPassword.matchPassword(password);
    if (!isMatch) {
      return res.status(401).json({ message: 'Incorrect password' });
    }

    // Delete Cloudinary photos for all attendance records that have one
    const attendanceWithPhotos = await Attendance.find({
      sessionId: session._id,
      photoPublicId: { $exists: true, $ne: '' },
    });

    if (attendanceWithPhotos.length > 0) {
      try {
        const storage = getStorageProvider();
        // allSettled so one failed deletion doesn't block the rest
        await Promise.allSettled(
          attendanceWithPhotos.map((record) => storage.delete(record.photoPublicId))
        );
      } catch (storageError) {
        if (process.env.NODE_ENV !== 'test') console.error('Storage cleanup error (non-fatal):', storageError.message);
      }
    }

    // Cascade: delete all attendance records then the session itself
    await Attendance.deleteMany({ sessionId: session._id });

    // Detach any short links that pointed to this session so they can be reattached
    await ShortLink.updateMany(
      { sessionId: session._id },
      { $set: { sessionId: null, isActive: false } }
    );

    await Session.findByIdAndDelete(session._id);

    res.json({ message: 'Session and all attendance records deleted successfully' });

  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getSessionTOTP = async (req, res) => {
  try {
    const session = await Session.findOne({
      _id: req.params.id,
      createdBy: req.admin._id,
    });

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }

    if (!session.totpEnabled) {
      return res.status(400).json({ message: 'TOTP not enabled for this session' });
    }

    const shortLink = await ShortLink.findOne({ sessionId: session._id, isActive: true });

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
      shortLink: shortLink ? shortLink.shortCode : null,
      sessionActive: session.isActive,
      // QR anti-sharing: 4-second rotating token for the QR URL
      qrToken: shortLink ? generateQRToken(shortLink.shortCode, session.totpSecret) : null,
      qrTokenWindowSeconds: 4,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getFlaggedAttendance = async (req, res) => {
  try {
    const { sessionId } = req.query;
    const query = { 
      deviceFlag: { $ne: null },
      sessionId: sessionId ? sessionId : { $exists: true }
    };
    
    if (sessionId) {
      const session = await Session.findOne({ _id: sessionId, createdBy: req.admin._id });
      if (!session) {
        return res.status(404).json({ message: 'Session not found' });
      }
    }
    
    const flaggedAttendance = await Attendance.find(query)
      .populate('sessionId', 'description expiresAt isActive')
      .sort({ capturedAt: -1 });
    
    res.json(flaggedAttendance);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const reviewAttendanceFlag = async (req, res) => {
  try {
    const { id } = req.params;
    const { reviewed } = req.body;
    
    const attendance = await Attendance.findById(id).populate('sessionId');
    
    if (!attendance) {
      return res.status(404).json({ message: 'Attendance record not found' });
    }
    
    if (!attendance.deviceFlag) {
      return res.status(400).json({ message: 'This record has no flags to review' });
    }
    
    attendance.flagReviewed = reviewed;
    attendance.flagReviewedBy = req.admin._id;
    attendance.flagReviewedAt = new Date();
    
    await attendance.save();
    
    res.json({ 
      message: reviewed ? 'Flag marked as reviewed' : 'Flag review removed',
      attendance 
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getDevicesForSession = async (req, res) => {
  try {
    const { sessionId } = req.params;
    
    const session = await Session.findOne({ _id: sessionId, createdBy: req.admin._id });
    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
    }
    
    const devices = await Device.find({ sessionId })
      .sort({ lastSeenAt: -1 });
    
    res.json(devices);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

module.exports = {
  createSession,
  getSessions,
  getSessionById,
  rotateToken,
  deactivateSession,
  getSessionAttendance,
  getSessionStats,
  deleteSession,
  getSessionTOTP,
  getFlaggedAttendance,
  reviewAttendanceFlag,
  getDevicesForSession,
};
