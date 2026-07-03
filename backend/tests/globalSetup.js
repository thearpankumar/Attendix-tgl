/**
 * globalSetup.js
 *
 * Runs ONCE before all test suites (in the main Jest process).
 * Starts a single MongoMemoryServer instance and exposes the URI via
 * process.env so every worker can connect to the same database binary
 * without spawning extra mongod processes.
 *
 * Why: Spinning up a new MongoMemoryServer per test file (the old pattern)
 * downloads / forks the mongod binary repeatedly, which is the #1 cause of
 * slow test runs.
 */

const { MongoMemoryServer } = require('mongodb-memory-server');

module.exports = async () => {
  const instance = await MongoMemoryServer.create();
  const uri = instance.getUri();

  // Expose to all Jest workers via environment variable
  process.env.MONGO_TEST_URI = uri;

  // Keep a reference so globalTeardown can stop it
  global.__MONGO_INSTANCE__ = instance;
};
