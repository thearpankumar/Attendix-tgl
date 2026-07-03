/**
 * dbSetup.js — runs via setupFilesAfterEnv in every Jest worker.
 *
 * Responsibilities:
 *  1. Connect mongoose to the shared MongoMemoryServer (URI from globalSetup)
 *  2. Mock CloudinaryProvider.upload so tests never make real HTTP calls
 *  3. Clean collections between tests
 *  4. Disconnect cleanly so Jest exits without open-handle warnings
 */

const mongoose = require('mongoose');
const { CloudinaryProvider } = require('../src/storage');

// ---------------------------------------------------------------------------
// Storage mock — intercept upload() so tests don't hit real Cloudinary/S3.
// getUploadUrl, getFileUrl, getName, delete are left as-is (they don't make
// real network calls on the Cloudinary provider in test mode).
// ---------------------------------------------------------------------------
jest.spyOn(CloudinaryProvider.prototype, 'upload').mockResolvedValue({
  url: 'https://res.cloudinary.com/test-cloud/image/upload/v1/attendance-photos/test-photo.jpg',
  publicId: 'attendance-photos/test-photo',
});

// ---------------------------------------------------------------------------
// Mongoose lifecycle
// ---------------------------------------------------------------------------
beforeAll(async () => {
  const uri = process.env.MONGO_TEST_URI;
  if (!uri) {
    throw new Error(
      'MONGO_TEST_URI is not set. Make sure tests/globalSetup.js ran correctly.'
    );
  }

  if (mongoose.connection.readyState === 0) {
    await mongoose.connect(uri, {
      maxPoolSize: 5,
      minPoolSize: 1,
    });
  }
});

afterAll(async () => {
  if (mongoose.connection.readyState !== 0) {
    await mongoose.disconnect();
  }
});

afterEach(async () => {
  // Wipe every collection after each test for isolation.
  // deleteMany({}) is faster than dropDatabase() (preserves indexes).
  const collections = mongoose.connection.collections;
  await Promise.all(Object.values(collections).map((c) => c.deleteMany({})));
});
