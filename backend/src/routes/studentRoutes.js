const express = require('express');
const router = express.Router();
const attendanceController = require('../controllers/attendanceController');
const { validateAttendance } = require('../middleware/validators');
const { studentLimiter } = require('../middleware/rateLimiter');
const { validateDeviceFingerprint, checkRapidSubmission } = require('../middleware/deviceCheck');
const { requireMobileDevice } = require('../middleware/mobileCheck');

router.get('/:token', requireMobileDevice, attendanceController.validateToken);
router.get('/:token/status', requireMobileDevice, attendanceController.checkAttendanceStatus);
router.get('/:token/upload-url', requireMobileDevice, attendanceController.getUploadUrl);
router.get('/:token/captcha', requireMobileDevice, attendanceController.getCaptcha);
router.post('/:token', 
  studentLimiter, 
  requireMobileDevice,
  validateAttendance, 
  validateDeviceFingerprint, 
  checkRapidSubmission, 
  attendanceController.submitAttendance
);

module.exports = router;
