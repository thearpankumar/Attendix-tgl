const express = require('express');
const cors = require('cors');
const helmet = require('helmet');
const compression = require('compression');
const connectDB = require('./config/db');
const config = require('./config');
const { initializeStorage } = require('./storage');

const app = express();

// Trust the nginx reverse proxy (1 hop).
// Without this, express-rate-limit throws ERR_ERL_UNEXPECTED_X_FORWARDED_FOR
// on every request that passes through nginx (all ngrok traffic), aborting them.
app.set('trust proxy', 1);

connectDB();

initializeStorage(config.storage);

app.use(helmet({
  contentSecurityPolicy: false,        // CSP on API JSON responses causes issues with browser module scripts behind ngrok
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

app.use('/api/admin', require('./routes/adminRoutes'));
app.use('/api/attend', require('./routes/studentRoutes'));

app.use((req, res, _next) => {
  res.status(404).json({ message: 'Route not found' });
});

app.use((err, req, res, _next) => {
  console.error(err.stack);
  const statusCode = err.statusCode || 500;
  res.status(statusCode).json({ 
    message: err.message || 'Something went wrong!',
    ...(config.nodeEnv === 'development' && { stack: err.stack })
  });
});

const PORT = config.port;
app.listen(PORT, () => {
  console.log(`Server running on port ${PORT}`);
  console.log(`Environment: ${config.nodeEnv}`);
});

module.exports = app;
