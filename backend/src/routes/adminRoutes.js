const express = require('express');
const router = express.Router();
const adminController = require('../controllers/adminController');
const locationController = require('../controllers/locationController');
const sessionController = require('../controllers/sessionController');
const { protect } = require('../middleware/auth');
const {
  validateAdmin,
  validateLogin,
  validateLocation,
  validateSession,
} = require('../middleware/validators');
const { loginLimiter, adminLimiter } = require('../middleware/rateLimiter');

router.post('/register', validateAdmin, adminController.createAdmin);
router.post('/login', loginLimiter, validateLogin, adminController.loginAdmin);
router.get('/profile', protect, adminController.getAdminProfile);
router.get('/dashboard', protect, adminController.getDashboardStats);

router.use(protect);
router.use(adminLimiter);

router.post('/locations', validateLocation, locationController.createLocation);
router.get('/locations', locationController.getLocations);
router.get('/locations/:id', locationController.getLocationById);
router.put('/locations/:id', validateLocation, locationController.updateLocation);
router.delete('/locations/:id', locationController.deleteLocation);

router.post('/sessions', validateSession, sessionController.createSession);
router.get('/sessions', sessionController.getSessions);
router.get('/sessions/:id', sessionController.getSessionById);
router.post('/sessions/:id/rotate', sessionController.rotateToken);
router.post('/sessions/:id/deactivate', sessionController.deactivateSession);
router.delete('/sessions/:id', sessionController.deleteSession);
router.get('/sessions/:id/attendance', sessionController.getSessionAttendance);
router.get('/sessions/:id/stats', sessionController.getSessionStats);

module.exports = router;
