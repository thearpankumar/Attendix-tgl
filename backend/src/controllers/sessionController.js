const Session = require('../models/Session');
const Location = require('../models/Location');
const Attendance = require('../models/Attendance');
const Admin = require('../models/Admin');
const { getStorageProvider } = require('../storage');

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
      .sort({ createdAt: -1 });

    const sessionsWithStats = await Promise.all(
      sessions.map(async (session) => {
        const attendanceCount = await Attendance.countDocuments({
          sessionId: session._id,
        });
        return {
          ...session.toObject(),
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

    // Find session — must belong to this admin
    const session = await Session.findOne({
      _id: req.params.id,
      createdBy: req.admin._id,
    });

    if (!session) {
      return res.status(404).json({ message: 'Session not found' });
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
        console.error('Storage cleanup error (non-fatal):', storageError.message);
      }
    }

    // Cascade: delete all attendance records then the session itself
    await Attendance.deleteMany({ sessionId: session._id });
    await Session.findByIdAndDelete(session._id);

    res.json({ message: 'Session and all attendance records deleted successfully' });
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
};
