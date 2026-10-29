use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub socks5: String,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            socks5: "socks5://10.96.1.108:7890".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnsConfig {
    pub port: u16,
    pub connect_ip_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub size: usize,
    pub serve_expired: bool,
    pub prefetch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedCheckConfig {
    pub enabled: bool,
    pub timeout_ms: u64,
    pub cache_mins: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamsConfig {
    pub mainland: Vec<String>,
    pub gfw: Vec<String>,
    pub overseas: Vec<String>,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub dns: DnsConfig,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub proxy: ProxyConfig,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub speed_check: SpeedCheckConfig,
    #[serde(default)]
    pub anti_pollution: bool,
    #[serde(default)]
    pub upstreams: UpstreamsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dns: DnsConfig {
                port: 53,
                connect_ip_version: "dual".to_string(),
            },
            web: WebConfig { port: 3000 },
            proxy: ProxyConfig::default(),
            cache: CacheConfig {
                size: 10000,
                serve_expired: true,
                prefetch: true,
            },
            speed_check: SpeedCheckConfig {
                enabled: true,
                timeout_ms: 400,
                cache_mins: 5,
            },
            anti_pollution: true,
            upstreams: UpstreamsConfig {
                mainland: vec![
                    "udp://223.5.5.5:53".to_string(),
                    "udp://119.29.29.29:53".to_string(),
                    "https://dns.alidns.com/dns-query".to_string(),
                ],
                gfw: vec![
                    "https://dns.google/dns-query".to_string(),
                    "https://cloudflare-dns.com/dns-query".to_string(),
                    "udp://8.8.8.8:53".to_string(),
                ],
                overseas: vec![
                    "udp://1.1.1.1:53".to_string(),
                    "https://1.1.1.1/dns-query".to_string(),
                    "udp://8.8.4.4:53".to_string(),
                ],
                timeout_ms: 2000,
            },
        }
    }
}

impl AppConfig {
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        if path.as_ref().exists() {
            let content = fs::read_to_string(&path)?;
            let config: Self = serde_yaml::from_str(&content)?;
            Ok(config)
        } else {
            let default_cfg = Self::default();
            if let Some(parent) = path.as_ref().parent() {
                fs::create_dir_all(parent)?;
            }
            let content = serde_yaml::to_string(&default_cfg)?;
            fs::write(&path, content)?;
            Ok(default_cfg)
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let content = serde_yaml::to_string(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
