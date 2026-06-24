require('dotenv').config({ path: '../.env' });
const cloudinary = require('cloudinary').v2;

async function testUpload() {
  console.log('Configuring Cloudinary with:');
  console.log('Cloud Name:', process.env.CLOUDINARY_CLOUD_NAME);
  console.log('API Key:', process.env.CLOUDINARY_API_KEY);
  console.log('API Secret (length):', process.env.CLOUDINARY_API_SECRET ? process.env.CLOUDINARY_API_SECRET.length : 0);

  cloudinary.config({
    cloud_name: process.env.CLOUDINARY_CLOUD_NAME,
    api_key: process.env.CLOUDINARY_API_KEY,
    api_secret: process.env.CLOUDINARY_API_SECRET,
    secure: true,
  });

  // A tiny valid 1x1 transparent GIF base64
  const dummyBase64 = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';

  try {
    console.log('Attempting upload of dummy 1x1 image to Cloudinary...');
    const result = await cloudinary.uploader.upload(dummyBase64, {
      folder: 'manual-test',
      resource_type: 'image',
    });
    console.log('Upload SUCCESSFUL! Result:');
    console.log(result);
  } catch (error) {
    console.error('Upload FAILED! Error details:');
    console.error(error);
  }
}

testUpload();
