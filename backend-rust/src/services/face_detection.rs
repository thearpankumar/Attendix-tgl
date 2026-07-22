use image::{DynamicImage, GenericImageView, ImageReader};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tracing::{debug, info, warn};

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaceDetectionResult {
    pub face_detected: bool,
    pub confidence: f64,
    pub bounding_box: Option<BoundingBox>,
    pub landmarks: Option<Vec<Landmark>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Landmark {
    pub x: f32,
    pub y: f32,
    pub label: String,
}

static FACE_ANALYZER: OnceCell<Option<face_id::analyzer::FaceAnalyzer>> = OnceCell::new();

async fn get_face_analyzer() -> Option<&'static face_id::analyzer::FaceAnalyzer> {
    FACE_ANALYZER
        .get_or_init(|| {
            match tokio::runtime::Handle::try_current() {
                Ok(rt) => {
                    let result = rt.block_on(async {
                        match face_id::analyzer::FaceAnalyzer::from_hf()
                            .detector_score_threshold(0.5)
                            .build()
                            .await
                        {
                            Ok(analyzer) => {
                                info!("Face detection ML model (SCRFD via ONNX Runtime) loaded successfully");
                                Some(analyzer)
                            }
                            Err(e) => {
                                warn!("Failed to load face detection ML model: {}. Falling back to basic detection.", e);
                                None
                            }
                        }
                    });
                    result
                }
                Err(_) => {
                    warn!("No tokio runtime available for face detection initialization");
                    None
                }
            }
        })
        .as_ref()
}

pub fn init_face_detector() {
    let _ = FACE_ANALYZER.get_or_init(|| None);
}

pub async fn detect_faces(image_data: &[u8]) -> Result<FaceDetectionResult> {
    let img = match ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .map_err(|e| AppError::BadRequest(format!("Invalid image format: {}", e)))?
        .decode()
    {
        Ok(img) => img,
        Err(e) => {
            debug!("Failed to decode image: {}", e);
            return Ok(FaceDetectionResult {
                face_detected: false,
                confidence: 0.0,
                bounding_box: None,
                landmarks: None,
                reason: Some("invalid_image_format".to_string()),
            });
        }
    };

    let (width, height) = img.dimensions();

    if width < 100 || height < 100 {
        debug!("Image too small for face detection: {}x{}", width, height);
        return Ok(FaceDetectionResult {
            face_detected: false,
            confidence: 0.0,
            bounding_box: None,
            landmarks: None,
            reason: Some("image_too_small".to_string()),
        });
    }

    if let Some(analyzer) = get_face_analyzer().await {
        detect_with_ml_model(analyzer, &img, width, height)
    } else {
        detect_with_fallback(&img, width, height)
    }
}

fn detect_with_ml_model(
    analyzer: &face_id::analyzer::FaceAnalyzer,
    img: &DynamicImage,
    width: u32,
    height: u32,
) -> Result<FaceDetectionResult> {
    let faces = analyzer
        .analyze(img)
        .map_err(|e| AppError::Internal(format!("Face detection failed: {}", e)))?;

    if faces.is_empty() {
        info!("No face detected by ML model");
        return Ok(FaceDetectionResult {
            face_detected: false,
            confidence: 0.0,
            bounding_box: None,
            landmarks: None,
            reason: Some("no_face_detected".to_string()),
        });
    }

    let best_face = faces
        .iter()
        .max_by(|a, b| {
            a.detection
                .score
                .partial_cmp(&b.detection.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap();

    let bbox_rel = &best_face.detection.bbox;

    let abs_x = bbox_rel.x1 * width as f32;
    let abs_y = bbox_rel.y1 * height as f32;
    let abs_width = (bbox_rel.x2 - bbox_rel.x1) * width as f32;
    let abs_height = (bbox_rel.y2 - bbox_rel.y1) * height as f32;

    let landmarks: Option<Vec<Landmark>> = best_face
        .detection
        .landmarks
        .as_ref()
        .map(|lms| {
            lms.iter()
                .enumerate()
                .map(|(i, lm)| {
                    let label = match i {
                        0 => "left_eye",
                        1 => "right_eye",
                        2 => "nose",
                        3 => "mouth_left",
                        4 => "mouth_right",
                        _ => "unknown",
                    };
                    Landmark {
                        x: lm.0 * width as f32,
                        y: lm.1 * height as f32,
                        label: label.to_string(),
                    }
                })
                .collect()
        });

    let confidence = best_face.detection.score;

    info!(
        "Face detected by ML model: confidence={:.2}, bbox=({:.0},{:.0},{:.0}x{:.0}), landmarks={}",
        confidence,
        abs_x,
        abs_y,
        abs_width,
        abs_height,
        landmarks.as_ref().map(|l| l.len()).unwrap_or(0)
    );

    Ok(FaceDetectionResult {
        face_detected: true,
        confidence: confidence as f64,
        bounding_box: Some(BoundingBox {
            x: abs_x,
            y: abs_y,
            width: abs_width,
            height: abs_height,
        }),
        landmarks,
        reason: Some("ml_detected".to_string()),
    })
}

fn detect_with_fallback(img: &DynamicImage, width: u32, height: u32) -> Result<FaceDetectionResult> {
    debug!("Using fallback face detection (image content analysis)");

    let rgb = img.to_rgb8();
    let total_pixels = (width * height) as f32;
    
    let mut skin_pixels = 0u32;
    let mut total_brightness = 0u64;

    for pixel in rgb.pixels() {
        let r = pixel[0] as f32;
        let g = pixel[1] as f32;
        let b = pixel[2] as f32;

        total_brightness += ((r + g + b) / 3.0) as u64;

        let is_skin = r > 95.0 && g > 40.0 && b > 20.0
            && r > g && r > b
            && (r - g).abs() > 15.0
            && r.max(g.max(b)) - r.min(g.min(b)) > 15.0;

        if is_skin {
            skin_pixels += 1;
        }
    }

    let skin_ratio = skin_pixels as f32 / total_pixels;
    let avg_brightness = total_brightness as f32 / total_pixels;

    let has_face_like_content = skin_ratio > 0.05 && skin_ratio < 0.6
        && avg_brightness > 40.0 && avg_brightness < 220.0;

    let confidence = if has_face_like_content {
        0.6 + (skin_ratio * 0.3).min(0.35)
    } else {
        0.0
    };

    if has_face_like_content {
        let face_factor = (confidence - 0.6) / 0.3;
        let face_width = width as f32 * 0.25 + face_factor * width as f32 * 0.25;
        let face_height = face_width * 1.2;
        let face_x = (width as f32 - face_width) / 2.0;
        let face_y = (height as f32 - face_height) / 2.0;

        info!(
            "Face detected by fallback heuristics: confidence={:.2}, skin_ratio={:.2}%",
            confidence,
            skin_ratio * 100.0
        );

        Ok(FaceDetectionResult {
            face_detected: true,
            confidence: confidence as f64,
            bounding_box: Some(BoundingBox {
                x: face_x,
                y: face_y,
                width: face_width,
                height: face_height,
            }),
            landmarks: None,
            reason: Some("fallback_detected".to_string()),
        })
    } else {
        info!(
            "No face detected (fallback): skin_ratio={:.2}%, avg_brightness={:.0}",
            skin_ratio * 100.0,
            avg_brightness
        );
        Ok(FaceDetectionResult {
            face_detected: false,
            confidence: 0.0,
            bounding_box: None,
            landmarks: None,
            reason: Some("no_face_detected".to_string()),
        })
    }
}

pub fn check_photo_reuse(image_data: &[u8], stored_hashes: &[u64], threshold: u32) -> Result<bool> {
    let hash = compute_image_hash(image_data)?;
    for &stored_hash in stored_hashes {
        let distance = compare_hashes(hash, stored_hash);
        if distance <= threshold {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn compute_image_hash(image_data: &[u8]) -> Result<u64> {
    let img = ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .map_err(|e| AppError::BadRequest(format!("Invalid image format: {}", e)))?
        .decode()
        .map_err(|e| AppError::BadRequest(format!("Failed to decode image: {}", e)))?;

    let resized = img.resize_exact(32, 32, image::imageops::FilterType::Lanczos3);
    let gray = resized.to_luma8();

    let pixels: Vec<u8> = gray.pixels().map(|p| p.0[0]).collect();
    let mean: f64 = pixels.iter().map(|&p| p as f64).sum::<f64>() / pixels.len() as f64;

    let mut hash: u64 = 0;
    for (i, &pixel) in pixels.iter().enumerate() {
        if i < 64 && pixel as f64 > mean {
            hash |= 1 << i;
        }
    }

    Ok(hash)
}

pub fn compare_hashes(hash1: u64, hash2: u64) -> u32 {
    (hash1 ^ hash2).count_ones()
}
