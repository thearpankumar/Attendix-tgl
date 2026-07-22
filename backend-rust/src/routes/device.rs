use axum::{extract::State, Json, Router};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{constants::*, error::Result, AppState};

pub fn create_routes() -> Router<Arc<AppState>> {
    Router::new().route("/verify", axum::routing::post(verify_device))
}

#[derive(Debug, Deserialize)]
pub struct DeviceMetrics {
    pub is_emulation: Option<bool>,
    pub inconsistencies: Option<Vec<String>>,
    pub webgl_renderer: Option<String>,
    pub device_memory: Option<i32>,
    pub screen_width: Option<i32>,
    pub screen_height: Option<i32>,
    pub platform: Option<String>,
    pub user_agent: Option<String>,
    pub client_hint_mobile: Option<String>,
    pub client_hint_platform: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct VerificationResponse {
    pub valid: bool,
    pub is_emulation: bool,
    pub inconsistencies: Vec<String>,
    pub message: String,
    pub confidence: f64,
}

static EMULATOR_GPU_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        r"SwiftShader",
        r"llvmpipe",
        r"EMU",
        r"VirtualBox",
        r"VMware",
        r"Microsoft Basic Render",
        r"GDI Generic",
        r"Hyper-V",
        r"Parallels",
        r"QEMU",
        r"Bochs",
        r"Android Emulator",
        r"BlueStacks",
        r"NoxApp",
        r"Genymotion",
        r"andy",
        r"andyous",
        r"andy_x86",
        r"MemuHT",
        r"Microvirt",
        r"Emulator",
    ]
    .iter()
    .filter_map(|p| Regex::new(p).ok())
    .collect()
});

static DESKTOP_GPU_PATTERNS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"NVIDIA|AMD|Radeon|GeForce|Intel.*Graphics|RTX|GTX|Arc").unwrap());

static MOBILE_USER_AGENT_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"Mobi|Android|iPhone|iPad|iPod|BlackBerry|Windows Phone|webOS|Opera Mini|IEMobile")
        .unwrap()
});

static CHROMIUM_BROWSER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"Chrome|Chromium|Edg|Opera|Brave").unwrap());

pub async fn verify_device(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<DeviceMetrics>,
) -> Result<Json<VerificationResponse>> {
    let mut inconsistencies: Vec<String> = Vec::new();
    let mut server_side_checks: Vec<String> = Vec::new();

    let (ua_claims_mobile, is_chromium) = if let Some(ref ua) = payload.user_agent {
        (
            MOBILE_USER_AGENT_PATTERNS.is_match(ua),
            CHROMIUM_BROWSER.is_match(ua),
        )
    } else {
        (false, false)
    };

    if is_chromium {
        if let Some(ref ch_mobile) = payload.client_hint_mobile {
            let ch_says_mobile = ch_mobile == "?1";
            if ua_claims_mobile && !ch_says_mobile {
                inconsistencies
                    .push("Server: UA claims mobile but Sec-CH-UA-Mobile disagrees".to_string());
            }
        }

        if let Some(ref ch_platform) = payload.client_hint_platform {
            let is_desktop_platform = ch_platform.contains("Windows")
                || ch_platform.contains("macOS")
                || ch_platform.contains("Linux")
                || ch_platform.contains("Chrome OS");
            if is_desktop_platform && ua_claims_mobile {
                inconsistencies.push(format!(
                    "Server: Desktop platform header '{}' with mobile UA",
                    ch_platform
                ));
            }
        }
    }

    if let Some(ref webgl_renderer) = payload.webgl_renderer {
        for pattern in EMULATOR_GPU_PATTERNS.iter() {
            if pattern.is_match(webgl_renderer) {
                server_side_checks.push(format!("Emulator GPU detected: {}", webgl_renderer));
                break;
            }
        }

        if DESKTOP_GPU_PATTERNS.is_match(webgl_renderer) && ua_claims_mobile {
            inconsistencies.push(format!(
                "Server: Desktop GPU '{}' detected with mobile User-Agent",
                webgl_renderer
            ));
        }
    }

    if let Some(ref platform) = payload.platform {
        let is_desktop = platform.to_lowercase().contains("win")
            || platform.to_lowercase().contains("mac")
            || platform.to_lowercase().contains("linux");
        if is_desktop && ua_claims_mobile && !platform.to_lowercase().contains("android") {
            inconsistencies.push(format!(
                "Server: Desktop platform '{}' with mobile User-Agent",
                platform
            ));
        }
    }

    if let Some(is_emulation) = payload.is_emulation {
        if is_emulation {
            server_side_checks.push("Client-side emulation patterns detected".to_string());
        }
    }

    if let Some(ref client_inconsistencies) = payload.inconsistencies {
        for inc in client_inconsistencies {
            server_side_checks.push(format!("Client: {}", inc));
        }
    }

    if let Some(device_memory) = payload.device_memory {
        if device_memory < 2 {
            server_side_checks.push(format!(
                "Suspicious device memory: {}GB (too low)",
                device_memory
            ));
        }
    }

    if let (Some(width), Some(height)) = (payload.screen_width, payload.screen_height) {
        if width == height && width > 0 {
            server_side_checks.push(format!(
                "Suspicious screen resolution: {}x{} (square)",
                width, height
            ));
        }
        if width < 320 || height < 320 {
            server_side_checks.push(format!(
                "Suspicious screen resolution: {}x{} (too small)",
                width, height
            ));
        }
    }

    let all_issues: Vec<String> = inconsistencies
        .into_iter()
        .chain(server_side_checks)
        .collect();

    let is_valid = all_issues.is_empty();
    let is_emulation = !is_valid;

    let confidence = if is_valid {
        0.95
    } else {
        (0.95 - (all_issues.len() as f64 * DEVICE_CONFIDENCE_PENALTY)).max(0.1)
    };

    Ok(Json(VerificationResponse {
        valid: is_valid,
        is_emulation,
        inconsistencies: all_issues,
        message: if is_valid {
            "Device verified".to_string()
        } else {
            "Device emulation detected. Please use a real mobile device.".to_string()
        },
        confidence,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mobile_user_agent_patterns() {
        assert!(MOBILE_USER_AGENT_PATTERNS.is_match("Mozilla/5.0 (Linux; Android 10; SM-G973F)"));
        assert!(MOBILE_USER_AGENT_PATTERNS
            .is_match("Mozilla/5.0 (iPhone; CPU iPhone OS 14_0 like Mac OS X)"));
        assert!(!MOBILE_USER_AGENT_PATTERNS.is_match("Mozilla/5.0 (Windows NT 10.0; Win64; x64)"));
    }
}
