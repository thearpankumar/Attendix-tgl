const mongoose = require('mongoose');
const Admin = require('../models/Admin');
const Location = require('../models/Location');
const Session = require('../models/Session');
const Attendance = require('../models/Attendance');
const Flag = require('../models/Flag');
const { generateToken } = require('../middleware/auth');
const config = require('../config');

const createAdmin = async (req, res) => {
  try {
    const { username, email, password, adminSecret } = req.body;

    if (adminSecret !== config.adminSecret) {
      return res.status(403).json({ message: 'Invalid admin secret' });
    }

    const existingAdmin = await Admin.findOne({
      $or: [{ username }, { email }],
    });

    if (existingAdmin) {
      return res.status(400).json({ message: 'Admin already exists' });
    }

    const admin = await Admin.create({
      username,
      email,
      password,
    });

    const token = generateToken(admin._id);

    res.status(201).json({
      admin: {
        _id: admin._id,
        username: admin.username,
        email: admin.email,
        role: admin.role,
      },
      token,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const loginAdmin = async (req, res) => {
  try {
    const { username, password } = req.body;

    const admin = await Admin.findOne({ username });

    if (!admin) {
      return res.status(401).json({ message: 'Invalid credentials' });
    }

    const isMatch = await admin.matchPassword(password);

    if (!isMatch) {
      return res.status(401).json({ message: 'Invalid credentials' });
    }

    const token = generateToken(admin._id);

    res.json({
      _id: admin._id,
      username: admin.username,
      email: admin.email,
      role: admin.role,
      token,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getAdminProfile = async (req, res) => {
  try {
    res.json({
      _id: req.admin._id,
      username: req.admin.username,
      email: req.admin.email,
      role: req.admin.role,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getDashboardStats = async (req, res) => {
  try {
    const adminSessions = await Session.find({ createdBy: req.admin._id }).select('_id');
    const sessionIds = adminSessions.map((s) => s._id);

    const [totalLocations, activeSessions, totalAttendance, flaggedUnreviewed] = await Promise.all([
      Location.countDocuments({ createdBy: req.admin._id }),
      Session.countDocuments({ createdBy: req.admin._id, isActive: true, expiresAt: { $gt: new Date() } }),
      Attendance.countDocuments({ sessionId: { $in: sessionIds } }),
      Flag.countDocuments({
        resolved: false,
        $or: [{ adminId: req.admin._id }, { sessionId: { $in: sessionIds } }],
      }),
    ]);

    res.json({ totalLocations, activeSessions, totalAttendance, flaggedUnreviewed });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

const getRecentActivity = async (req, res) => {
  try {
    const adminSessionIds = await Session.find({ createdBy: req.admin._id }).distinct('_id');
    const records = await Attendance.find({ sessionId: { $in: adminSessionIds } })
      .sort({ capturedAt: -1 })
      .limit(5)
      .populate({ path: 'sessionId', populate: { path: 'locationId', select: 'name' } })
      .select('studentName rollNumber capturedAt verified sessionId');

    const activity = records.map((r) => ({
      studentName: r.studentName,
      rollNumber: r.rollNumber,
      locationName: r.sessionId?.locationId?.name || 'Unknown',
      capturedAt: r.capturedAt,
      verified: r.verified,
    }));

    res.json(activity);
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

// Daily attendance counts per session, scoped to this admin's locations.
// Returns flat rows { date, location, session, count } — the frontend buckets
// these into daily/weekly/monthly and stacks by session.
const getAttendanceSeries = async (req, res) => {
  try {
    const { locationId } = req.query;
    const days = Math.min(parseInt(req.query.days) || 180, 730);
    const from = new Date();
    from.setDate(from.getDate() - days);
    from.setHours(0, 0, 0, 0);

    const locationMatch = { 'location.createdBy': req.admin._id };
    if (locationId) {
      if (!mongoose.Types.ObjectId.isValid(String(locationId))) {
        return res.status(400).json({ message: 'Invalid locationId' });
      }
      locationMatch['location._id'] = new mongoose.Types.ObjectId(String(locationId));
    }

    const rows = await Attendance.aggregate([
      { $match: { capturedAt: { $gte: from } } },
      { $lookup: { from: 'sessions', localField: 'sessionId', foreignField: '_id', as: 'session' } },
      { $unwind: '$session' },
      { $lookup: { from: 'locations', localField: 'session.locationId', foreignField: '_id', as: 'location' } },
      { $unwind: '$location' },
      { $match: locationMatch },
      {
        $group: {
          _id: {
            day: { $dateToString: { format: '%Y-%m-%d', date: '$capturedAt' } },
            session: '$session._id',
          },
          count: { $sum: 1 },
          sessionLabel: { $first: '$session.description' },
          location: { $first: '$location.name' },
        },
      },
      { $sort: { '_id.day': 1 } },
    ]);

    res.json(rows.map((r) => ({
      date: r._id.day,
      location: r.location,
      session: r.sessionLabel || 'Session',
      count: r.count,
    })));
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

// Sessions that recorded attendance on a given date (UTC), with student counts.
// Powers the calendar-driven dashboard: pick a date -> see that day's sessions.
const getSessionsByDate = async (req, res) => {
  try {
    const { date } = req.query; // YYYY-MM-DD
    if (!/^\d{4}-\d{2}-\d{2}$/.test(date || '')) {
      return res.status(400).json({ message: 'Valid date (YYYY-MM-DD) required' });
    }

    const rows = await Attendance.aggregate([
      { $addFields: { day: { $dateToString: { format: '%Y-%m-%d', date: '$capturedAt' } } } },
      { $match: { day: date } },
      { $lookup: { from: 'sessions', localField: 'sessionId', foreignField: '_id', as: 'session' } },
      { $unwind: '$session' },
      { $lookup: { from: 'locations', localField: 'session.locationId', foreignField: '_id', as: 'location' } },
      { $unwind: '$location' },
      { $match: { 'location.createdBy': req.admin._id } },
      {
        $group: {
          _id: '$session._id',
          description: { $first: '$session.description' },
          location: { $first: '$location.name' },
          time: { $first: '$session.createdAt' },
          count: { $sum: 1 },
        },
      },
      { $sort: { time: 1 } },
    ]);

    res.json(rows.map((r) => ({
      sessionId: r._id,
      description: r.description || 'Session',
      location: r.location,
      time: r.time,
      count: r.count,
    })));
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

module.exports = {
  createAdmin,
  loginAdmin,
  getAdminProfile,
  getDashboardStats,
  getRecentActivity,
  getAttendanceSeries,
  getSessionsByDate,
};
