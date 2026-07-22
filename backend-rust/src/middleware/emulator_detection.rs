#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmulatorIndicators {
    pub desktop_gpu_detected: bool,
    pub audio_fingerprint_emulator: bool,
    pub timing_anomaly: bool,
    pub battery_pattern_emulator: bool,
    pub screen_resolution_suspicious: bool,
    pub device_memory_round: bool,
    pub webgl_renderer_emulator: bool,
    pub platform_inconsistency: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulatorFlagResult {
    pub flag_type: String,
    pub severity: String,
    pub details: String,
}

impl EmulatorIndicators {
    pub fn from_user_agent_info(user_agent: &str, webgl_renderer: Option<&str>) -> Self {
        let mut indicators = Self::default();

        let emulator_patterns = [
            "Genymotion",
            "BlueStacks",
            "Nox",
            "LDPlayer",
            "AndroidSDK",
            "emu",
        ];

        for pattern in emulator_patterns {
            if user_agent.contains(pattern) {
                indicators.platform_inconsistency = true;
            }
        }

        if let Some(renderer) = webgl_renderer {
            let desktop_gpu_patterns = ["NVIDIA", "GeForce", "Radeon", "Intel(R) HD"];
            for pattern in desktop_gpu_patterns {
                if renderer.contains(pattern) {
                    indicators.desktop_gpu_detected = true;
                    break;
                }
            }

            // Expanded GPU patterns to detect emulators/VMs from Node.js backend
            let emulator_renderer_patterns = [
                "SwiftShader",         // Chrome emulators
                "llvmpipe",           // Linux software rendering
                "Mesa",               // QEMU, Linux VMs
                "VMware",             // VMware desktop VMs
                "VirtualBox",         // VirtualBox desktop VMs
                "Microsoft Basic Render", // Hyper-V
                "Gallium",            // Linux VM drivers
                "Zink",               // OpenGL over Vulkan (VMs)
                "virgl",              // Virtio GPU
                "nouveau",            // NVIDIA open-source driver (VMs)
                "softpipe",           // Software rendering
                "Microsoft Remote Display Adapter", // RDP
                "superflower",        // Android emulator
                " WASTD",              // Trailing space intentional
                "paravirtualized",    // Hyper-V paravirtualized
                "Cirrus Logic",       // QEMU legacy
                "QEMU",               // QEMU
                "vbox",               // VirtualBox abbreviated
                "vmware",             // VMware lowercase
                "vmsvga",             // VMware SVGA
            ];
            for pattern in emulator_renderer_patterns {
                if renderer.contains(pattern) {
                    indicators.webgl_renderer_emulator = true;
                    break;
                }
            }
        }

        indicators
    }

    pub fn check_timing(analyze_timing: f64) -> Option<EmulatorFlagResult> {
        if !(1.0..=5000.0).contains(&analyze_timing) {
            return Some(EmulatorFlagResult {
                flag_type: "TIMING_ANOMALY".to_string(),
                severity: "medium".to_string(),
                details: format!(
                    "Timing analysis returned suspicious value: {}ms",
                    analyze_timing
                ),
            });
        }
        None
    }

    pub fn check_screen(width: i32, height: i32) -> Option<EmulatorFlagResult> {
        let _typical_mobile_heights = [
            480, 568, 667, 736, 812, 844, 896, 926, 1080, 1280, 1440, 1920, 2160, 2400, 2560, 3200,
        ];

        let is_suspicious = width > 1000 || height > 1000;
        if is_suspicious {
            return Some(EmulatorFlagResult {
                flag_type: "SCREEN_RESOLUTION_SUSPICIOUS".to_string(),
                severity: "low".to_string(),
                details: format!("Non-mobile resolution: {}x{}", width, height),
            });
        }
        None
    }

    pub fn to_flags(&self) -> Vec<EmulatorFlagResult> {
        let mut flags = Vec::new();

        if self.desktop_gpu_detected {
            flags.push(EmulatorFlagResult {
                flag_type: "DESKTOP_GPU_DETECTED".to_string(),
                severity: "high".to_string(),
                details: "Desktop GPU detected on mobile device".to_string(),
            });
        }

        if self.audio_fingerprint_emulator {
            flags.push(EmulatorFlagResult {
                flag_type: "AUDIO_FINGERPRINT_EMULATOR".to_string(),
                severity: "medium".to_string(),
                details: "Audio fingerprint matches emulator profile".to_string(),
            });
        }

        if self.webgl_renderer_emulator {
            flags.push(EmulatorFlagResult {
                flag_type: "WEBGL_RENDERER_EMULATOR".to_string(),
                severity: "high".to_string(),
                details: "WebGL renderer indicates emulator".to_string(),
            });
        }

        if self.platform_inconsistency {
            flags.push(EmulatorFlagResult {
                flag_type: "PLATFORM_INCONSISTENCY".to_string(),
                severity: "medium".to_string(),
                details: "Platform inconsistencies detected".to_string(),
            });
        }

        flags
    }
}

pub fn is_emulator(user_agent: &str, webgl_renderer: Option<&str>) -> bool {
    let indicators = EmulatorIndicators::from_user_agent_info(user_agent, webgl_renderer);
    indicators.desktop_gpu_detected
        || indicators.webgl_renderer_emulator
        || indicators.platform_inconsistency
}
