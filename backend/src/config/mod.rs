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
    #[serde(default = "default_block_type")]
    pub block_type: String,
    #[serde(default = "default_block_ttl")]
    pub block_ttl: u32,
}

impl Default for DnsConfig {
    fn default() -> Self {
        Self {
            port: 53,
            connect_ip_version: "dual".to_string(),
            block_type: "zero_ip".to_string(),
            block_ttl: 60,
        }
    }
}

fn default_block_type() -> String {
    "zero_ip".to_string()
}

fn default_block_ttl() -> u32 {
    60
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    pub port: u16,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self { port: 3000 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    pub size: usize,
    pub serve_expired: bool,
    pub prefetch: bool,
    #[serde(default = "default_min_ttl")]
    pub min_ttl: u32,
    #[serde(default = "default_max_ttl")]
    pub max_ttl: u32,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            size: 10000,
            serve_expired: true,
            prefetch: true,
            min_ttl: 60,
            max_ttl: 3600,
        }
    }
}

fn default_min_ttl() -> u32 {
    60
}

fn default_max_ttl() -> u32 {
    3600
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedCheckConfig {
    pub enabled: bool,
    pub timeout_ms: u64,
    pub cache_mins: u64,
    #[serde(default = "default_speed_mode")]
    pub mode: String,
}

impl Default for SpeedCheckConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_ms: 400,
            cache_mins: 5,
            mode: "tcp_80_443".to_string(),
        }
    }
}

fn default_speed_mode() -> String {
    "tcp_80_443".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamsConfig {
    pub mainland: Vec<String>,
    pub gfw: Vec<String>,
    pub overseas: Vec<String>,
    pub timeout_ms: u64,
}

impl Default for UpstreamsConfig {
    fn default() -> Self {
        Self {
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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingConfig {
    pub anti_pollution: bool,
    pub qtype_65_reject: bool,
    pub fallback_timeout_ms: u64,
}

impl Default for RoutingConfig {
    fn default() -> Self {
        Self {
            anti_pollution: true,
            qtype_65_reject: true,
            fallback_timeout_ms: 1500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub retention_days: u32,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            retention_days: 7,
        }
    }
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
    pub routing: RoutingConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub upstreams: UpstreamsConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dns: DnsConfig {
                port: 53,
                connect_ip_version: "dual".to_string(),
                block_type: "zero_ip".to_string(),
                block_ttl: 60,
            },
            web: WebConfig { port: 3000 },
            proxy: ProxyConfig::default(),
            cache: CacheConfig {
                size: 10000,
                serve_expired: true,
                prefetch: true,
                min_ttl: 60,
                max_ttl: 3600,
            },
            speed_check: SpeedCheckConfig {
                enabled: true,
                timeout_ms: 400,
                cache_mins: 5,
                mode: "tcp_80_443".to_string(),
            },
            anti_pollution: true,
            routing: RoutingConfig::default(),
            logging: LoggingConfig::default(),
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
