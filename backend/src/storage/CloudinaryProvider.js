const StorageProvider = require('./StorageProvider');
const cloudinary = require('cloudinary').v2;

class CloudinaryProvider extends StorageProvider {
  constructor(config) {
    super();
    this.config = config;
    cloudinary.config({
      cloud_name: config.cloudName,
      api_key: config.apiKey,
      api_secret: config.apiSecret,
      secure: true,
    });
  }

  async upload(file, options = {}) {
    try {
      const result = await cloudinary.uploader.upload(file, {
        folder: options.folder || 'attendance-photos',
        resource_type: 'image',
        transformation: [
          { quality: 'auto:good' },
          { width: 800, height: 800, crop: 'limit' },
        ],
      });
      return {
        url: result.secure_url,
        publicId: result.public_id,
        provider: 'cloudinary',
      };
    } catch (error) {
      throw new Error(`Cloudinary upload failed: ${error.message}`, { cause: error });
    }
  }

  async delete(publicId) {
    try {
      await cloudinary.uploader.destroy(publicId);
      return true;
    } catch (error) {
      throw new Error(`Cloudinary delete failed: ${error.message}`, { cause: error });
    }
  }

  getFileUrl(publicId) {
    return cloudinary.url(publicId, { secure: true });
  }

  async getUploadUrl(key, _contentType) {
    const timestamp = Math.round(new Date().getTime() / 1000);
    const signature = cloudinary.utils.api_sign_request(
      { timestamp, folder: 'attendance-photos' },
      this.config.apiSecret
    );
    
    return {
      uploadUrl: `https://api.cloudinary.com/v1_1/${this.config.cloudName}/image/upload`,
      publicId: `attendance-photos/${key}`,
      params: {
        api_key: this.config.apiKey,
        timestamp,
        signature,
      },
      method: 'POST',
    };
  }

  getName() {
    return 'cloudinary';
  }
}

module.exports = CloudinaryProvider;
