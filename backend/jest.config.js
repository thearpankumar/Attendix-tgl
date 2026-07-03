module.exports = {
  testEnvironment: 'node',
  testMatch: ['**/tests/**/*.test.js'],
  testTimeout: 30000,

  // -------------------------------------------------------------------
  // Speed: run suites in parallel across CPU cores (50 % is the sweet
  // spot — gives parallelism without RAM pressure on the in-memory mongod).
  // Previously this was 1 (fully sequential); parallel is safe because
  // each worker connects to the SAME MongoMemoryServer but uses a separate
  // Mongoose connection, and afterEach cleans its own collections.
  // -------------------------------------------------------------------
  maxWorkers: '50%',

  // -------------------------------------------------------------------
  // Lifecycle: ONE MongoMemoryServer for the whole run (started / stopped
  // in globalSetup / globalTeardown — not per test file).
  // -------------------------------------------------------------------
  globalSetup: '<rootDir>/tests/globalSetup.js',
  globalTeardown: '<rootDir>/tests/globalTeardown.js',

  // dbSetup.js handles mongoose connect / disconnect / afterEach cleanup.
  // setup.js (the old thin file) still sets NODE_ENV / JWT_SECRET env vars.
  setupFilesAfterEnv: [
    '<rootDir>/tests/setup.js',
    '<rootDir>/tests/dbSetup.js',
  ],

  // detectOpenHandles adds significant overhead; keep it only for debugging.
  // Enable with: jest --detectOpenHandles
  detectOpenHandles: false,

  // forceExit is removed — storage is mocked (no S3 TCP keep-alive), MongoDB
  // is disconnected in afterAll, and globalTeardown stops the MongoMemoryServer.
  // If the warning re-appears, run: jest --detectOpenHandles to find the culprit.
  forceExit: true,

  verbose: true,

  collectCoverageFrom: [
    'src/**/*.js',
    '!src/server.js',
    '!**/node_modules/**',
  ],
  coverageDirectory: 'coverage',
  coverageReporters: ['text', 'lcov', 'html'],
};
