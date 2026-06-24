require('dotenv').config();

const requiredEnvVars = ['JWT_SECRET', 'ADMIN_SECRET'];

const storageProvider = process.env.STORAGE_PROVIDER || 'cloudinary';

if (process.env.NODE_ENV === 'production') {
  requiredEnvVars.forEach((envVar) => {
    if (!process.env[envVar]) {
      throw new Error(`Missing required environment variable: ${envVar}`);
    }
  });

  if (storageProvider === 's3' || storageProvider === 'aws') {
    const s3EnvVars = ['AWS_S3_BUCKET', 'AWS_ACCESS_KEY_ID', 'AWS_SECRET_ACCESS_KEY'];
    s3EnvVars.forEach((envVar) => {
      if (!process.env[envVar]) {
        throw new Error(`Missing required S3 environment variable: ${envVar}`);
      }
    });
  } else {
    const cloudinaryEnvVars = ['CLOUDINARY_CLOUD_NAME', 'CLOUDINARY_API_KEY', 'CLOUDINARY_API_SECRET'];
    cloudinaryEnvVars.forEach((envVar) => {
      if (!process.env[envVar]) {
        console.warn(`Warning: Missing Cloudinary env var ${envVar} - image uploads may fail`);
      }
    });
  }
}

module.exports = {
  port: parseInt(process.env.PORT) || 5000,
  mongodbUri: process.env.MONGODB_URI || 'mongodb://localhost:27017/attendance-geotag',
  jwtSecret: process.env.JWT_SECRET || 'dev-secret-change-in-production',
  jwtExpire: process.env.JWT_EXPIRE || '7d',
  adminSecret: process.env.ADMIN_SECRET || 'dev-admin-secret',
  nodeEnv: process.env.NODE_ENV || 'development',
  corsOrigin: process.env.CORS_ORIGIN || '*',
  
  storage: {
    provider: storageProvider,
    
    cloudinary: {
      cloudName: process.env.CLOUDINARY_CLOUD_NAME || '',
      apiKey: process.env.CLOUDINARY_API_KEY || '',
      apiSecret: process.env.CLOUDINARY_API_SECRET || '',
    },
    
    s3: {
      bucket: process.env.AWS_S3_BUCKET || '',
      region: process.env.AWS_REGION || 'us-east-1',
      accessKeyId: process.env.AWS_ACCESS_KEY_ID || '',
      secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY || '',
    },
  },
  
  cloudinary: {
    cloudName: process.env.CLOUDINARY_CLOUD_NAME || '',
    apiKey: process.env.CLOUDINARY_API_KEY || '',
    apiSecret: process.env.CLOUDINARY_API_SECRET || '',
  },
};
