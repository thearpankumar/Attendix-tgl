class StorageProvider {
  async upload(_file, _options) {
    throw new Error('Method not implemented');
  }

  async delete(_publicId) {
    throw new Error('Method not implemented');
  }

  getFileUrl(_publicId) {
    throw new Error('Method not implemented');
  }

  async getUploadUrl(_key, _contentType) {
    throw new Error('Method not implemented');
  }

  getName() {
    throw new Error('Method not implemented');
  }
}

module.exports = StorageProvider;
