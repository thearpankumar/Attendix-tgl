const StorageProvider = require('./StorageProvider');
const { S3Client, PutObjectCommand, DeleteObjectCommand, GetObjectCommand } = require('@aws-sdk/client-s3');
const { getSignedUrl } = require('@aws-sdk/s3-request-presigner');

class S3Provider extends StorageProvider {
  constructor(config) {
    super();
    this.config = config;
    this.bucket = config.bucket;
    this.region = config.region || 'us-east-1';
    
    this.s3Client = new S3Client({
      region: this.region,
      credentials: {
        accessKeyId: config.accessKeyId,
        secretAccessKey: config.secretAccessKey,
      },
    });
  }

  async upload(file, options = {}) {
    try {
      let buffer;
      let contentType = 'image/jpeg';
      
      if (file.startsWith('data:')) {
        const matches = file.match(/^data:(.+);base64,(.+)$/);
        if (matches) {
          contentType = matches[1];
          buffer = Buffer.from(matches[2], 'base64');
        } else {
          throw new Error('Invalid data URL format');
        }
      } else {
        buffer = Buffer.from(file, 'base64');
      }

      const key = `${options.folder || 'attendance-photos'}/${options.key || Date.now()}.jpg`;
      
      const command = new PutObjectCommand({
        Bucket: this.bucket,
        Key: key,
        Body: buffer,
        ContentType: contentType,
        CacheControl: 'max-age=31536000',
      });

      await this.s3Client.send(command);

      const url = `https://${this.bucket}.s3.${this.region}.amazonaws.com/${key}`;
      
      return {
        url,
        publicId: key,
        provider: 's3',
      };
    } catch (error) {
      throw new Error(`S3 upload failed: ${error.message}`, { cause: error });
    }
  }

  async delete(publicId) {
    try {
      const command = new DeleteObjectCommand({
        Bucket: this.bucket,
        Key: publicId,
      });
      await this.s3Client.send(command);
      return true;
    } catch (error) {
      throw new Error(`S3 delete failed: ${error.message}`, { cause: error });
    }
  }

  getFileUrl(publicId) {
    return `https://${this.bucket}.s3.${this.region}.amazonaws.com/${publicId}`;
  }

  async getUploadUrl(key, contentType = 'image/jpeg') {
    try {
      const objectKey = `attendance-photos/${key}`;
      
      const command = new PutObjectCommand({
        Bucket: this.bucket,
        Key: objectKey,
        ContentType: contentType,
      });

      const uploadUrl = await getSignedUrl(this.s3Client, command, {
        expiresIn: 300,
      });

      return {
        uploadUrl,
        publicId: objectKey,
        method: 'PUT',
        contentType,
        headers: {
          'Content-Type': contentType,
        },
      };
    } catch (error) {
      throw new Error(`Failed to generate upload URL: ${error.message}`, { cause: error });
    }
  }

  async getDownloadUrl(publicId, expiresIn = 3600) {
    try {
      const command = new GetObjectCommand({
        Bucket: this.bucket,
        Key: publicId,
      });

      return await getSignedUrl(this.s3Client, command, { expiresIn });
    } catch (error) {
      throw new Error(`Failed to generate download URL: ${error.message}`, { cause: error });
    }
  }

  getName() {
    return 's3';
  }
}

module.exports = S3Provider;
