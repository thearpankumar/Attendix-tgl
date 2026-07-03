const Session = require('../models/Session');
const { isRedisConnected, getRedisClient } = require('../config/redis');

const SESSION_CACHE_PREFIX = 'session:';
const CACHE_TTL = 300;

async function getCachedSession(tokenHash) {
  if (!isRedisConnected()) {
    return await Session.findOne({
      tokenHash,
      isActive: true,
      expiresAt: { $gt: new Date() }
    }).populate('locationId', 'name latitude longitude radiusMeters');
  }

  const redis = getRedisClient();
  const cacheKey = `${SESSION_CACHE_PREFIX}${tokenHash}`;
  
  try {
    const cached = await redis.get(cacheKey);
    
    if (cached) {
      const sessionData = JSON.parse(cached);
      
      if (sessionData.locationId && typeof sessionData.locationId === 'string') {
        return await Session.findOne({
          tokenHash,
          isActive: true,
          expiresAt: { $gt: new Date() }
        }).populate('locationId', 'name latitude longitude radiusMeters');
      }
      
      return sessionData;
    }
    
    const session = await Session.findOne({
      tokenHash,
      isActive: true,
      expiresAt: { $gt: new Date() }
    }).populate('locationId', 'name latitude longitude radiusMeters');
    
    if (session) {
      await redis.setex(cacheKey, CACHE_TTL, JSON.stringify(session.toObject()));
    }
    
    return session;
  } catch (error) {
    console.error('Redis cache error:', error.message);
    
    return await Session.findOne({
      tokenHash,
      isActive: true,
      expiresAt: { $gt: new Date() }
    }).populate('locationId', 'name latitude longitude radiusMeters');
  }
}

async function invalidateSessionCache(tokenHash) {
  if (!isRedisConnected()) {
    return;
  }

  const redis = getRedisClient();
  const cacheKey = `${SESSION_CACHE_PREFIX}${tokenHash}`;
  
  try {
    await redis.del(cacheKey);
    console.log(`Cache invalidated for token: ${tokenHash.substring(0, 8)}...`);
  } catch (error) {
    console.error('Redis cache invalidation error:', error.message);
  }
}

async function getSessionStats(tokenHash) {
  if (!isRedisConnected()) {
    return { cached: false, source: 'database' };
  }

  const redis = getRedisClient();
  const cacheKey = `${SESSION_CACHE_PREFIX}${tokenHash}`;
  
  try {
    const ttl = await redis.ttl(cacheKey);
    const exists = await redis.exists(cacheKey);
    
    return {
      cached: exists === 1,
      ttl: ttl > 0 ? ttl : 0,
      source: exists === 1 ? 'cache' : 'database'
    };
  } catch (error) {
    return { cached: false, source: 'database' };
  }
}

module.exports = {
  getCachedSession,
  invalidateSessionCache,
  getSessionStats
};
