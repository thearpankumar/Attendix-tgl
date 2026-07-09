const rateLimit = require('express-rate-limit');

// Evaluate per-request so tests that set NODE_ENV=test after module load are covered.
const isTest = () => process.env.NODE_ENV === 'test';

const adminLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 100,
  standardHeaders: true,
  legacyHeaders: false,
  skip: isTest,
  message: {
    message: 'Too many requests, please try again later',
  },
});

const studentLimiter = rateLimit({
  windowMs: 60 * 1000,
  max: 20,
  standardHeaders: true,
  legacyHeaders: false,
  skipFailedRequests: true,
  skip: isTest,
  message: {
    message: 'Too many submissions, please wait',
  },
});

const loginLimiter = rateLimit({
  windowMs: 15 * 60 * 1000,
  max: 5,
  standardHeaders: true,
  legacyHeaders: false,
  skip: isTest,
  message: {
    message: 'Too many login attempts, please try again later',
  },
});

module.exports = {
  adminLimiter,
  studentLimiter,
  loginLimiter,
};
