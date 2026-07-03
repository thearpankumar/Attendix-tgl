const Admin = require('../models/Admin');
const Location = require('../models/Location');
const Session = require('../models/Session');
const Attendance = require('../models/Attendance');
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
    const totalLocations = await Location.countDocuments({ createdBy: req.admin._id });
    const activeSessions = await Session.countDocuments({
      createdBy: req.admin._id,
      isActive: true,
      expiresAt: { $gt: new Date() },
    });
    const totalAttendance = await Attendance.countDocuments();

    res.json({
      totalLocations,
      activeSessions,
      totalAttendance,
    });
  } catch (error) {
    res.status(500).json({ message: 'Server error', error: error.message });
  }
};

module.exports = {
  createAdmin,
  loginAdmin,
  getAdminProfile,
  getDashboardStats,
};
