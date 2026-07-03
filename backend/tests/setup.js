// Global test setup — runs in every Jest worker via setupFilesAfterEnv,
// BEFORE any test file or application module is loaded.
//
// Critical ordering: these env vars must be set here so that when any test
// file does require('../src/server'), the config module reads these values
// instead of the production .env values (dotenv does NOT override existing
// process.env keys, so setting them first wins).

process.env.NODE_ENV    = 'test';
process.env.JWT_SECRET  = 'test-jwt-secret-for-testing';
process.env.ADMIN_SECRET = 'test-admin-secret';

// Force cloudinary with fake creds — prevents a real S3 client from being
// created, which is the root cause of:
//   1. "NoSuchBucket" errors hitting real AWS during tests
//   2. "Force exiting Jest" warnings (S3 HTTP agent keeps connections open)
process.env.STORAGE_PROVIDER        = 'cloudinary';
process.env.CLOUDINARY_CLOUD_NAME   = 'test-cloud';
process.env.CLOUDINARY_API_KEY      = 'test-api-key';
process.env.CLOUDINARY_API_SECRET   = 'test-api-secret';
