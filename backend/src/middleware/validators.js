const { body, validationResult } = require('express-validator');

const handleValidationErrors = (req, res, next) => {
  const errors = validationResult(req);
  if (!errors.isEmpty()) {
    return res.status(400).json({
      message: 'Validation failed',
      errors: errors.array().map(err => ({
        field: err.path,
        message: err.msg,
      })),
    });
  }
  next();
};

const validateAdmin = [
  body('username')
    .trim()
    .isLength({ min: 3, max: 30 })
    .withMessage('Username must be 3-30 characters')
    .isAlphanumeric()
    .withMessage('Username must be alphanumeric'),
  body('email')
    .isEmail()
    .normalizeEmail()
    .withMessage('Valid email required'),
  body('password')
    .isLength({ min: 6 })
    .withMessage('Password must be at least 6 characters'),
  handleValidationErrors,
];

const validateLogin = [
  body('username').trim().notEmpty().withMessage('Username required'),
  body('password').notEmpty().withMessage('Password required'),
  handleValidationErrors,
];

const validateLocation = [
  body('name')
    .trim()
    .escape()                         // Sanitize HTML/script tags (XSS prevention)
    .isLength({ min: 1, max: 100 })
    .withMessage('Location name must be 1-100 characters'),
  body('latitude')
    .isFloat({ min: -90, max: 90 })
    .withMessage('Valid latitude required'),
  body('longitude')
    .isFloat({ min: -180, max: 180 })
    .withMessage('Valid longitude required'),
  body('radiusMeters')
    .optional()
    .isInt({ min: 10, max: 10000 })
    .withMessage('Radius must be 10-10000 meters'),
  handleValidationErrors,
];

const validateSession = [
  body('locationId')
    .isMongoId()
    .withMessage('Valid location ID required'),
  body('durationMinutes')
    .optional()
    .isInt({ min: 5, max: 480 })
    .withMessage('Duration must be 5-480 minutes'),
  handleValidationErrors,
];

const validateAttendance = [
  body('studentName')
    .trim()
    .isLength({ min: 2, max: 100 })
    .withMessage('Name must be 2-100 characters'),
  body('rollNumber')
    .trim()
    .isLength({ min: 1, max: 20 })
    .withMessage('Roll number required')
    .custom(value => /^[a-zA-Z0-9]+$/.test(value))
    .withMessage('Roll number must be alphanumeric'),
  body('directUpload')
    .optional()
    .isBoolean()
    .withMessage('directUpload must be boolean'),
  body('publicId')
    .if(body('directUpload').equals('true'))
    .notEmpty()
    .withMessage('publicId required when using direct upload'),
  body('photo')
    .if(body('directUpload').not().equals('true'))
    .notEmpty()
    .withMessage('Photo required')
    .custom(value => {
      if (value && !value.startsWith('data:image/')) {
        throw new Error('Invalid photo format');
      }
      return true;
    }),
  body('latitude')
    .isFloat({ min: -90, max: 90 })
    .withMessage('Valid latitude required'),
  body('longitude')
    .isFloat({ min: -180, max: 180 })
    .withMessage('Valid longitude required'),
  body('faceDetected')
    .optional()
    .isBoolean()
    .withMessage('faceDetected must be boolean'),
  body('captchaAnswer')
    .if(() => process.env.NODE_ENV !== 'test')
    .trim()
    .notEmpty()
    .withMessage('Captcha answer required')
    .isAlphanumeric()
    .withMessage('Captcha answer must be alphanumeric'),
  body('captchaId')
    .if(() => process.env.NODE_ENV !== 'test')
    .trim()
    .notEmpty()
    .withMessage('Captcha ID required'),
  handleValidationErrors,
];

module.exports = {
  handleValidationErrors,
  validateAdmin,
  validateLogin,
  validateLocation,
  validateSession,
  validateAttendance,
};
