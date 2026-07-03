const Redis = require('ioredis');
const config = require('./index');

let redisClient = null;

function initializeRedis() {
  if (!config.redis?.url) {
    console.warn('Redis URL not configured - caching disabled');
    return null;
  }

  try {
    redisClient = new Redis(config.redis.url, {
      maxRetriesPerRequest: 3,
      retryDelayOnFailover: 100,
      enableReadyCheck: true,
      lazyConnect: false,
      reconnectOnError: (err) => {
        const targetError = 'READONLY';
        if (err.message.includes(targetError)) {
          return true;
        }
        return false;
      },
    });

    redisClient.on('connect', () => {
      console.log('✓ Redis: Connected');
    });

    redisClient.on('ready', () => {
      console.log('✓ Redis: Ready to accept commands');
    });

    redisClient.on('error', (err) => {
      console.error('✗ Redis error:', err.message);
    });

    redisClient.on('close', () => {
      console.warn('⚠ Redis: Connection closed');
    });

    redisClient.on('reconnecting', () => {
      console.log('↻ Redis: Reconnecting...');
    });

    return redisClient;
  } catch (error) {
    console.error('✗ Redis initialization failed:', error.message);
    return null;
  }
}

function getRedisClient() {
  if (!redisClient) {
    throw new Error('Redis not initialized. Call initializeRedis first.');
  }
  return redisClient;
}

function isRedisConnected() {
  return redisClient && redisClient.status === 'ready';
}

async function closeRedis() {
  if (redisClient) {
    await redisClient.quit();
    redisClient = null;
    console.log('✓ Redis: Connection closed gracefully');
  }
}

module.exports = {
  initializeRedis,
  getRedisClient,
  isRedisConnected,
  closeRedis
};
