const express = require('express');
const cors = require('cors');
const helmet = require('helmet');
const compression = require('compression');
const connectDB = require('./config/db');
const { initializeRedis, closeRedis, isRedisConnected } = require('./config/redis');
const config = require('./config');
const { initializeStorage } = require('./storage');

const app = express();

app.set('trust proxy', 1);

if (config.nodeEnv !== 'test') {
  connectDB();
  initializeRedis();
}

// Initialize storage in all environments (including test) so that
// /api/storage-info and attendance endpoints work correctly.
try {
  initializeStorage(config.storage);
} catch (storageErr) {
  // In test env the credentials are fake — that's expected.
  // Errors only matter at upload time, not at initialization.
  console.warn('Storage init warning:', storageErr.message);
}

app.use(helmet({
  hsts: false,                         // HSTS handled by ngrok/nginx, not needed from the API
}));
app.use(compression());
app.use(express.json({ limit: '10mb' }));
app.use(express.urlencoded({ extended: true }));

app.use(
  cors({
    origin: config.corsOrigin,
    credentials: true,
  })
);

app.get('/health', (req, res) => {
  res.json({ status: 'OK', timestamp: new Date().toISOString() });
});

app.get('/api/storage-info', (req, res) => {
  const { getStorageProvider } = require('./storage');
  try {
    const storage = getStorageProvider();
    res.json({
      provider: storage.getName(),
      supportsDirectUpload: typeof storage.getUploadUrl === 'function',
    });
  } catch (_error) {
    res.json({ provider: 'not-initialized' });
  }
});

app.get('/health/ready', (req, res) => {
  const redisStatus = isRedisConnected() ? 'connected' : 'disconnected';
  res.json({
    status: 'ready',
    timestamp: new Date().toISOString(),
    redis: redisStatus
  });
});

app.get('/health/live', (req, res) => {
  res.json({ status: 'alive', timestamp: new Date().toISOString() });
});

app.use('/api/admin', require('./routes/adminRoutes'));
app.use('/api/attend', require('./routes/studentRoutes'));
app.use('/s', require('./routes/shortLinkRoutes'));
app.use('/s', require('./routes/webauthnRoutes'));

app.use((req, res, _next) => {
  res.status(404).json({ message: 'Route not found' });
});

app.use((err, req, res, _next) => {
  if (process.env.NODE_ENV !== 'test') console.error(err.stack);
  const statusCode = err.statusCode || 500;
  res.status(statusCode).json({ 
    message: err.message || 'Something went wrong!',
    ...(config.nodeEnv === 'development' && { stack: err.stack })
  });
});

if (config.nodeEnv !== 'test') {
  const PORT = config.port;
  const server = app.listen(PORT, () => {
    console.log(`Server running on port ${PORT}`);
    console.log(`Environment: ${config.nodeEnv}`);
  });

  process.on('SIGTERM', async () => {
    console.log('SIGTERM signal received: closing server');
    server.close(() => {
      console.log('HTTP server closed');
    });
    await closeRedis();
    process.exit(0);
  });

  process.on('SIGINT', async () => {
    console.log('SIGINT signal received: closing server');
    server.close(() => {
      console.log('HTTP server closed');
    });
    await closeRedis();
    process.exit(0);
  });
}

module.exports = app;
