const StorageProvider = require('../src/storage/StorageProvider');
const CloudinaryProvider = require('../src/storage/CloudinaryProvider');
const S3Provider = require('../src/storage/S3Provider');

describe('StorageProvider Interface', () => {
  test('should throw error for unimplemented methods', async () => {
    const provider = new StorageProvider();
    
    await expect(provider.upload()).rejects.toThrow('Method not implemented');
    await expect(provider.delete()).rejects.toThrow('Method not implemented');
    expect(() => provider.getFileUrl()).toThrow('Method not implemented');
    await expect(provider.getUploadUrl()).rejects.toThrow('Method not implemented');
    expect(() => provider.getName()).toThrow('Method not implemented');
  });
});

describe('CloudinaryProvider', () => {
  let provider;
  // cloudinary module is a singleton — spy on it directly.
  const cloudinary = require('cloudinary').v2;

  beforeEach(() => {
    provider = new CloudinaryProvider({
      cloudName: 'test-cloud',
      apiKey: 'test-key',
      apiSecret: 'test-secret',
    });
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  test('should initialize with config', () => {
    expect(provider.config.cloudName).toBe('test-cloud');
  });

  test('should return correct provider name', () => {
    expect(provider.getName()).toBe('cloudinary');
  });

  // --- upload() ---

  test('should upload file and return url + publicId', async () => {
    jest.spyOn(cloudinary.uploader, 'upload').mockResolvedValueOnce({
      secure_url: 'https://res.cloudinary.com/test-cloud/image/upload/v1/attendance-photos/photo.jpg',
      public_id: 'attendance-photos/photo',
    });

    const result = await provider.upload('data:image/jpeg;base64,/9j==', {
      folder: 'attendance-photos',
    });

    expect(result.url).toBe(
      'https://res.cloudinary.com/test-cloud/image/upload/v1/attendance-photos/photo.jpg'
    );
    expect(result.publicId).toBe('attendance-photos/photo');
    expect(result.provider).toBe('cloudinary');
    // Verify cloudinary SDK was called with the right folder
    expect(cloudinary.uploader.upload).toHaveBeenCalledWith(
      'data:image/jpeg;base64,/9j==',
      expect.objectContaining({ folder: 'attendance-photos' })
    );
  });

  test('should apply image quality + resize transformation on upload', async () => {
    jest.spyOn(cloudinary.uploader, 'upload').mockResolvedValueOnce({
      secure_url: 'https://res.cloudinary.com/test-cloud/image/upload/v1/photo.jpg',
      public_id: 'photo',
    });

    await provider.upload('data:image/jpeg;base64,abc', {});

    const callArgs = cloudinary.uploader.upload.mock.calls[0][1];
    expect(callArgs.transformation).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ quality: 'auto:good' }),
        expect.objectContaining({ width: 800, height: 800, crop: 'limit' }),
      ])
    );
  });

  test('should wrap upload errors with descriptive message', async () => {
    jest.spyOn(cloudinary.uploader, 'upload').mockRejectedValueOnce(
      new Error('Invalid credentials')
    );

    await expect(
      provider.upload('data:image/jpeg;base64,abc', {})
    ).rejects.toThrow('Cloudinary upload failed: Invalid credentials');
  });

  // --- delete() ---

  test('should delete file by publicId', async () => {
    jest.spyOn(cloudinary.uploader, 'destroy').mockResolvedValueOnce({ result: 'ok' });

    const result = await provider.delete('attendance-photos/photo');
    expect(result).toBe(true);
    expect(cloudinary.uploader.destroy).toHaveBeenCalledWith('attendance-photos/photo');
  });

  test('should wrap delete errors with descriptive message', async () => {
    jest.spyOn(cloudinary.uploader, 'destroy').mockRejectedValueOnce(
      new Error('Resource not found')
    );

    await expect(
      provider.delete('attendance-photos/missing')
    ).rejects.toThrow('Cloudinary delete failed: Resource not found');
  });

  // --- getUploadUrl() ---

  test('should generate upload URL with required fields', async () => {
    const result = await provider.getUploadUrl('test-key', 'image/jpeg');

    expect(result).toHaveProperty('uploadUrl');
    expect(result).toHaveProperty('publicId');
    expect(result).toHaveProperty('params');
    expect(result.params).toHaveProperty('api_key');
    expect(result.params).toHaveProperty('timestamp');
    expect(result.params).toHaveProperty('signature');
    expect(result.method).toBe('POST');
  });

  test('should generate upload URL with correct publicId format', async () => {
    const result = await provider.getUploadUrl('student123', 'image/jpeg');
    expect(result.publicId).toBe('attendance-photos/student123');
  });

  // --- getFileUrl() ---

  test('should generate file URL containing cloud name and publicId', () => {
    const url = provider.getFileUrl('attendance-photos/test');
    expect(url).toContain('test-cloud');
    expect(url).toContain('attendance-photos/test');
  });
});

describe('S3Provider', () => {
  let provider;

  beforeEach(() => {
    provider = new S3Provider({
      bucket: 'test-bucket',
      region: 'us-east-1',
      accessKeyId: 'test-key',
      secretAccessKey: 'test-secret',
    });
  });

  afterEach(() => {
    jest.restoreAllMocks();
  });

  test('should initialize with config', () => {
    expect(provider.bucket).toBe('test-bucket');
    expect(provider.region).toBe('us-east-1');
  });

  test('should return correct provider name', () => {
    expect(provider.getName()).toBe('s3');
  });

  // --- getFileUrl() ---

  test('should generate correct file URL', () => {
    const url = provider.getFileUrl('attendance-photos/test.jpg');
    expect(url).toBe('https://test-bucket.s3.us-east-1.amazonaws.com/attendance-photos/test.jpg');
  });

  test('should generate file URL with custom region', () => {
    const customProvider = new S3Provider({
      bucket: 'test-bucket',
      region: 'eu-west-1',
      accessKeyId: 'test-key',
      secretAccessKey: 'test-secret',
    });
    expect(customProvider.getFileUrl('test.jpg')).toContain('eu-west-1');
  });

  // --- upload() happy path ---

  test('should upload a data-URL file and return url + publicId + provider', async () => {
    // Mock the S3 client's send() to avoid real AWS calls.
    jest.spyOn(provider.s3Client, 'send').mockResolvedValueOnce({});

    const dataUrl = 'data:image/jpeg;base64,/9j/4AAQSkZJRgABAQAAAQABAAD==';
    const result = await provider.upload(dataUrl, {
      folder: 'attendance-photos',
      key: 'student_21CS101_1234567890',
    });

    expect(result.url).toMatch(/^https:\/\/test-bucket\.s3\.us-east-1\.amazonaws\.com\/.+/);
    expect(result.publicId).toContain('attendance-photos/');
    expect(result.provider).toBe('s3');
    expect(provider.s3Client.send).toHaveBeenCalledTimes(1);
  });

  test('should use the folder + key from options in the S3 object key', async () => {
    jest.spyOn(provider.s3Client, 'send').mockResolvedValueOnce({});

    await provider.upload('data:image/jpeg;base64,abc123', {
      folder: 'attendance-photos',
      key: 'myfile',
    });

    // The PutObjectCommand passed to send() should reference the constructed key.
    const sentCommand = provider.s3Client.send.mock.calls[0][0];
    expect(sentCommand.input.Bucket).toBe('test-bucket');
    expect(sentCommand.input.Key).toMatch(/attendance-photos\/myfile/);
    expect(sentCommand.input.ContentType).toBe('image/jpeg');
  });

  test('should wrap S3 send errors in a descriptive S3 error', async () => {
    jest.spyOn(provider.s3Client, 'send').mockRejectedValueOnce(
      new Error('Access Denied')
    );

    await expect(
      provider.upload('data:image/jpeg;base64,abc123', {})
    ).rejects.toThrow('S3 upload failed: Access Denied');
  });

  // --- upload() error cases (no network needed) ---

  test('should reject upload when file is null', async () => {
    await expect(provider.upload(null, {})).rejects.toThrow();
  });

  test('should reject upload with malformed data: prefix URL', async () => {
    // Strings starting with 'data:' but not matching the full base64 pattern
    // trigger the 'Invalid data URL format' branch before any network call.
    await expect(
      provider.upload('data:image/jpeg;INVALID_FORMAT', {})
    ).rejects.toThrow('Invalid data URL format');
  });

  test('should reject upload when S3 send fails (non data: string path)', async () => {
    // Non-data: strings are treated as raw base64 and decoded locally, then
    // sent to S3. Mock send() to verify the error is wrapped correctly.
    jest.spyOn(provider.s3Client, 'send').mockRejectedValueOnce(
      new Error('The access key does not exist')
    );

    await expect(
      provider.upload('not-a-valid-data-url', {})
    ).rejects.toThrow('S3 upload failed: The access key does not exist');
  });

  test('should reject upload with empty file', async () => {
    await expect(provider.upload('', {})).rejects.toThrow();
  });

  // --- delete() ---

  test('should delete object by key', async () => {
    jest.spyOn(provider.s3Client, 'send').mockResolvedValueOnce({});

    const result = await provider.delete('attendance-photos/photo.jpg');
    expect(result).toBe(true);

    const sentCommand = provider.s3Client.send.mock.calls[0][0];
    expect(sentCommand.input.Bucket).toBe('test-bucket');
    expect(sentCommand.input.Key).toBe('attendance-photos/photo.jpg');
  });

  test('should wrap delete errors with descriptive message', async () => {
    jest.spyOn(provider.s3Client, 'send').mockRejectedValueOnce(
      new Error('NoSuchKey')
    );

    await expect(
      provider.delete('attendance-photos/missing.jpg')
    ).rejects.toThrow('S3 delete failed: NoSuchKey');
  });

  // --- getUploadUrl() (presigned URL) ---

  test('should generate a presigned upload URL for PUT', async () => {
    // getSignedUrl is called internally; with fake creds it generates a signed URL string.
    const result = await provider.getUploadUrl('student_001', 'image/jpeg');

    expect(result.method).toBe('PUT');
    expect(result.publicId).toBe('attendance-photos/student_001');
    expect(result.contentType).toBe('image/jpeg');
    expect(result.uploadUrl).toMatch(/^https:\/\/.+\.amazonaws\.com\/.+/);
    expect(result.headers).toEqual({ 'Content-Type': 'image/jpeg' });
  });

  test('should have all required provider methods', () => {
    expect(typeof provider.upload).toBe('function');
    expect(typeof provider.delete).toBe('function');
    expect(typeof provider.getUploadUrl).toBe('function');
    expect(typeof provider.getFileUrl).toBe('function');
    expect(typeof provider.getName).toBe('function');
  });
});

describe('Storage Factory', () => {
  beforeEach(() => {
    jest.resetModules();
  });

  test('should initialize Cloudinary provider by default', () => {
    const { initializeStorage } = require('../src/storage');
    
    const provider = initializeStorage({
      provider: 'cloudinary',
      cloudinary: {
        cloudName: 'test',
        apiKey: 'test',
        apiSecret: 'test',
      },
    });

    expect(provider.getName()).toBe('cloudinary');
  });

  test('should initialize S3 provider when specified', () => {
    const { initializeStorage } = require('../src/storage');
    
    const provider = initializeStorage({
      provider: 's3',
      s3: {
        bucket: 'test-bucket',
        accessKeyId: 'test-key',
        secretAccessKey: 'test-secret',
      },
    });

    expect(provider.getName()).toBe('s3');
  });

  test('should throw error for missing S3 config', () => {
    const { initializeStorage } = require('../src/storage');
    
    expect(() => initializeStorage({
      provider: 's3',
      s3: {},
    })).toThrow('S3 configuration incomplete');
  });

  test('should throw error for missing Cloudinary config', () => {
    const { initializeStorage } = require('../src/storage');
    
    expect(() => initializeStorage({
      provider: 'cloudinary',
      cloudinary: {},
    })).toThrow('Cloudinary configuration incomplete');
  });
});
