use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::pipeline::plugins::domain_match::{DomainListRegistry, DomainMatcher};
use crate::pipeline::plugins::ip_match::{IpListRegistry, IpMatcher};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleFileInfo {
    pub name: String,
    pub file_name: String,
    pub count: usize,
    pub size_bytes: u64,
    pub updated_at: String,
    pub url: String,
}

pub struct RuleFileManager {
    base_dir: PathBuf,
    domain_registry: Arc<DomainListRegistry>,
    ip_registry: Arc<IpListRegistry>,
}

impl RuleFileManager {
    pub fn new<P: AsRef<Path>>(
        base_dir: P,
        domain_registry: Arc<DomainListRegistry>,
        ip_registry: Arc<IpListRegistry>,
    ) -> Self {
        let p = base_dir.as_ref().to_path_buf();
        let _ = fs::create_dir_all(&p);
        Self {
            base_dir: p,
            domain_registry,
            ip_registry,
        }
    }

    pub fn list_rules(&self) -> Vec<RuleFileInfo> {
        let defaults = vec![
            (
                "大陆直连域名 (direct.txt)",
                "direct.txt",
                "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/direct.txt",
            ),
            (
                "代理过滤域名 (proxy.txt)",
                "proxy.txt",
                "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/proxy.txt",
            ),
            (
                "局域私有域名 (private.txt)",
                "private.txt",
                "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/private.txt",
            ),
            (
                "中国 IPv4 网段 (ipv4_china.txt)",
                "ipv4_china.txt",
                "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv4_china.txt",
            ),
            (
                "中国 IPv6 网段 (ipv6_china.txt)",
                "ipv6_china.txt",
                "https://gx.aorz.kdns.fr/https://raw.githubusercontent.com/217heidai/RoutingRules/main/rules/ipv6_china.txt",
            ),
        ];

        let mut result = Vec::new();
        for (name, file_name, url) in defaults {
            let file_path = self.base_dir.join(file_name);
            let (count, size, updated) = if file_path.exists() {
                let metadata = fs::metadata(&file_path).ok();
                let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                let updated = metadata
                    .and_then(|m| m.modified().ok())
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
                let line_count = fs::read_to_string(&file_path)
                    .map(|s| s.lines().filter(|l| !l.trim().is_empty() && !l.starts_with('#')).count())
                    .unwrap_or(0);
                (line_count, size, updated)
            } else {
                (0, 0, "-".to_string())
            };

            result.push(RuleFileInfo {
                name: name.to_string(),
                file_name: file_name.to_string(),
                count,
                size_bytes: size,
                updated_at: updated,
                url: url.to_string(),
            });
        }
        result
    }

    pub async fn update_rule_file(&self, file_name: &str, url: &str) -> anyhow::Result<usize> {
        let resp = reqwest::get(url).await?;
        let text = resp.text().await?;

        let file_path = self.base_dir.join(file_name);
        fs::write(&file_path, &text)?;

        let line_count = text
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
            .count();

        self.reload_file(file_name, &text);
        Ok(line_count)
    }

    pub async fn update_all(&self) -> Vec<(String, Result<usize, String>)> {
        let files = self.list_rules();
        let mut res = Vec::new();
        for f in files {
            match self.update_rule_file(&f.file_name, &f.url).await {
                Ok(cnt) => res.push((f.file_name, Ok(cnt))),
                Err(e) => res.push((f.file_name, Err(e.to_string()))),
            }
        }
        res
    }

    pub fn load_local_rules(&self) {
        let files = [
            "direct.txt",
            "proxy.txt",
            "private.txt",
            "ipv4_china.txt",
            "ipv6_china.txt",
        ];

        for f in files {
            let path = self.base_dir.join(f);
            if path.exists() {
                if let Ok(text) = fs::read_to_string(&path) {
                    self.reload_file(f, &text);
                }
            }
        }
    }

    fn reload_file(&self, file_name: &str, text: &str) {
        match file_name {
            "direct.txt" => {
                let mut m = DomainMatcher::new("domain_direct");
                m.load_from_text(text);
                self.domain_registry.register(m);
            }
            "proxy.txt" => {
                let mut m = DomainMatcher::new("domain_proxy");
                m.load_from_text(text);
                self.domain_registry.register(m);
            }
            "private.txt" => {
                let mut m = DomainMatcher::new("domain_private");
                m.load_from_text(text);
                self.domain_registry.register(m);
            }
            "ipv4_china.txt" => {
                let mut m = IpMatcher::new("ipv4_china");
                m.load_from_text(text);
                self.ip_registry.register(m);
            }
            "ipv6_china.txt" => {
                let mut m = IpMatcher::new("ipv6_china");
                m.load_from_text(text);
                self.ip_registry.register(m);
            }
            _ => {}
        }
    }
}
