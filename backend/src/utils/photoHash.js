const { createHash } = require('crypto');

async function computePerceptualHash(imageBuffer) {
  try {
    const sharp = require('sharp');
    
    const processed = await sharp(imageBuffer)
      .resize(32, 32, { fit: 'fill' })
      .grayscale()
      .raw()
      .toBuffer();
    
    let hash = '';
    for (let i = 0; i < processed.length - 1; i++) {
      hash += processed[i] > processed[i + 1] ? '1' : '0';
    }
    
    return createHash('sha256').update(hash).digest('hex');
  } catch (error) {
    throw new Error(`Failed to compute photo hash: ${error.message}`, { cause: error });
  }
}

async function validateImage(imageBuffer) {
  try {
    const sharp = require('sharp');
    const meta = await sharp(imageBuffer).metadata();
    
    if (imageBuffer.length > 5 * 1024 * 1024) {
      throw new Error('Image too large (max 5MB)');
    }
    
    if (meta.width < 200 || meta.height < 200) {
      throw new Error('Image resolution too low (min 200x200)');
    }
    
    if (meta.width > 4000 || meta.height > 4000) {
      throw new Error('Image resolution too high (max 4000x4000)');
    }
    
    if (!['jpeg', 'png', 'webp'].includes(meta.format)) {
      throw new Error('Invalid image format (only JPEG, PNG, WebP allowed)');
    }
    
    return { valid: true, format: meta.format, width: meta.width, height: meta.height };
  } catch (error) {
    if (error.message.includes('Image too large') || 
        error.message.includes('resolution') ||
        error.message.includes('format')) {
      throw error;
    }
    throw new Error('Invalid or corrupted image', { cause: error });
  }
}

async function sanitizeImage(imageBuffer) {
  try {
    const sharp = require('sharp');
    
    return await sharp(imageBuffer)
      .rotate()
      .jpeg({ quality: 85, mozjpeg: true })
      .toBuffer();
  } catch (error) {
    throw new Error(`Failed to sanitize image: ${error.message}`, { cause: error });
  }
}

function hammingDistance(hash1, hash2) {
  if (hash1.length !== hash2.length) return Infinity;
  
  let distance = 0;
  for (let i = 0; i < hash1.length; i++) {
    if (hash1[i] !== hash2[i]) distance++;
  }
  return distance;
}

module.exports = {
  computePerceptualHash,
  validateImage,
  sanitizeImage,
  hammingDistance,
};
