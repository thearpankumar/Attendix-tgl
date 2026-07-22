use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpInfo {
    pub isp: String,
    pub org: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

/// Look up IP information from an external API service.
///
/// # Timeouts
/// - Uses a 2-second per-call timeout (shorter than global client timeout)
/// - This allows faster fallback when the IP API is slow/unavailable
/// - The global http_client has a 30-second timeout as a safety net
pub async fn lookup_ip(client: &reqwest::Client, ip: &str) -> Result<IpInfo> {
    if is_private_ip(ip) {
        return Ok(IpInfo {
            isp: "Local Network".to_string(),
            org: "Local".to_string(),
            country: None,
            region: None,
            city: None,
        });
    }

    let api_url =
        std::env::var("IP_API_URL").unwrap_or_else(|_| "http://ip-api.com/json/".to_string());
    let url = format!(
        "{}{}?fields=status,message,isp,org,country,regionName,city",
        api_url, ip
    );

    match client
        .get(&url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<IpApiResponse>().await {
                    Ok(data) => {
                        if data.status == "success" {
                            Ok(IpInfo {
                                isp: data.isp.unwrap_or_else(|| "Unknown".to_string()),
                                org: data.org.unwrap_or_else(|| "Unknown".to_string()),
                                country: data.country,
                                region: data.region,
                                city: data.city,
                            })
                        } else {
                            Ok(unknown())
                        }
                    }
                    Err(_) => Ok(unknown()),
                }
            } else {
                Ok(unknown())
            }
        }
        Err(_) => Ok(unknown()),
    }
}

fn unknown() -> IpInfo {
    IpInfo {
        isp: "Unknown".to_string(),
        org: "Unknown".to_string(),
        country: None,
        region: None,
        city: None,
    }
}

fn is_private_ip(ip: &str) -> bool {
    ip == "::1"
        || ip == "127.0.0.1"
        || ip.starts_with("::ffff:")
        || ip.starts_with("192.168.")
        || ip.starts_with("10.")
        || is_172_private(ip)
}

fn is_172_private(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    if let (Ok(first), Ok(second)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
        return first == 172 && (16..=31).contains(&second);
    }
    false
}

#[derive(Debug, Deserialize)]
struct IpApiResponse {
    status: String,
    #[serde(default)]
    isp: Option<String>,
    #[serde(default)]
    org: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    #[serde(rename = "regionName")]
    region: Option<String>,
    #[serde(default)]
    city: Option<String>,
}

pub struct IpLookupService;

impl IpLookupService {
    pub fn new() -> Self {
        Self
    }

    pub async fn lookup(&self, client: &reqwest::Client, ip: &str) -> Result<IpInfo> {
        lookup_ip(client, ip).await
    }
}

impl Default for IpLookupService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_private_ip_detection() {
        assert!(is_private_ip("127.0.0.1"));
        assert!(is_private_ip("192.168.1.1"));
        assert!(is_private_ip("10.0.0.1"));
        assert!(is_private_ip("172.16.0.1"));
        assert!(!is_private_ip("8.8.8.8"));
    }
}
