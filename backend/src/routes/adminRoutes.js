const express = require('express');
const router = express.Router();
const adminController = require('../controllers/adminController');
const locationController = require('../controllers/locationController');
const sessionController = require('../controllers/sessionController');
const shortLinkController = require('../controllers/shortLinkController');
const webauthnController = require('../controllers/webauthnController');
const { protect } = require('../middleware/auth');
const {
  validateAdmin,
  validateLogin,
  validateLocation,
  validateSession,
} = require('../middleware/validators');
const { loginLimiter, adminLimiter } = require('../middleware/rateLimiter');

router.post('/register', loginLimiter, validateAdmin, adminController.createAdmin);
router.post('/login', loginLimiter, validateLogin, adminController.loginAdmin);

router.use(protect);
router.use(adminLimiter);

router.get('/profile', adminController.getAdminProfile);
router.get('/dashboard', adminController.getDashboardStats);
router.get('/dashboard/recent-activity', adminController.getRecentActivity);
router.get('/dashboard/attendance-series', adminController.getAttendanceSeries);
router.get('/dashboard/sessions-by-date', adminController.getSessionsByDate);

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
router.get('/sessions/:id/totp', sessionController.getSessionTOTP);
router.get('/sessions/:id/devices', sessionController.getDevicesForSession);

router.get('/flagged', sessionController.getFlaggedAttendance);
router.patch('/attendance/:id/review', sessionController.reviewAttendanceFlag);

router.post('/shortlinks', shortLinkController.createShortLink);
router.get('/shortlinks', shortLinkController.getShortLinks);
router.get('/shortlinks/available-sessions', shortLinkController.getAvailableSessions);
router.get('/shortlinks/:shortCode', shortLinkController.getShortLinkByCode);
router.post('/shortlinks/:shortCode/attach', shortLinkController.attachShortLinkToSession);
router.post('/shortlinks/:shortCode/detach', shortLinkController.detachShortLink);
router.delete('/shortlinks/:shortCode', shortLinkController.deleteShortLink);

router.post('/webauthn/reset', webauthnController.resetCredential);
router.post('/webauthn/suspend', webauthnController.suspendCredential);
router.post('/webauthn/unsuspend', webauthnController.unsuspendCredential);
router.get('/webauthn/credentials', webauthnController.getCredentials);
router.get('/webauthn/stats', webauthnController.getWebAuthnStats);

module.exports = router;
