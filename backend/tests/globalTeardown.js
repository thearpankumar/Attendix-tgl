/**
 * globalTeardown.js
 *
 * Runs ONCE after all test suites complete.
 * Shuts down the shared MongoMemoryServer started in globalSetup.js.
 */

module.exports = async () => {
  if (global.__MONGO_INSTANCE__) {
    await global.__MONGO_INSTANCE__.stop();
  }
};
