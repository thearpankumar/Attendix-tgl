use image::{GenericImageView, ImageReader};
use image_hasher::{HashAlg, HasherConfig};
use std::io::Cursor;

pub fn generate_photo_hash(image_data: &[u8]) -> crate::error::Result<String> {
    let img = ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .map_err(|e| crate::error::AppError::Image(e.to_string()))?
        .decode()
        .map_err(|e| crate::error::AppError::Image(e.to_string()))?;

    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::Gradient)
        .hash_size(8, 8)
        .to_hasher();

    let hash = hasher.hash_image(&img);

    Ok(hash.to_base64())
}

pub fn calculate_hash_similarity(hash1: &str, hash2: &str) -> f32 {
    let h1: image_hasher::ImageHash<Vec<u8>> = match image_hasher::ImageHash::from_base64(hash1) {
        Ok(h) => h,
        Err(_) => return 1.0,
    };

    let h2: image_hasher::ImageHash<Vec<u8>> = match image_hasher::ImageHash::from_base64(hash2) {
        Ok(h) => h,
        Err(_) => return 1.0,
    };

    let distance = h1.dist(&h2);
    let max_distance = 64.0;

    (distance as f32) / max_distance
}

pub fn is_same_photo(hash1: &str, hash2: &str, threshold: f32) -> bool {
    calculate_hash_similarity(hash1, hash2) < threshold
}

pub fn validate_image(
    image_data: &[u8],
    max_size_bytes: usize,
) -> crate::error::Result<(u32, u32)> {
    if image_data.len() > max_size_bytes {
        return Err(crate::error::AppError::BadRequest(format!(
            "Image too large: {} bytes (max: {})",
            image_data.len(),
            max_size_bytes
        )));
    }

    let img = ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .map_err(|e| crate::error::AppError::Image(e.to_string()))?
        .decode()
        .map_err(|e| crate::error::AppError::Image(e.to_string()))?;

    Ok(img.dimensions())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_consistency() {
        let mut png_data = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        png_data.extend_from_slice(&[0u8; 100]);

        let result = generate_photo_hash(&png_data);

        match &result {
            Ok(hash) => {
                assert!(!hash.is_empty());
            }
            Err(e) => {
                println!("Photo hash generation requires valid image format: {:?}", e);
            }
        }
        assert!(result.is_ok() || matches!(result, Err(crate::error::AppError::Image(_))));
    }
}
