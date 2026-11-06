use chrono::{Duration, Local};
use sha2::Digest;
use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;
use uuid::Uuid;
use crate::config::AuthConfig;

static SESSION_CACHE: LazyLock<RwLock<HashMap<String, (String, String)>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

static LOGIN_ATTEMPTS: LazyLock<RwLock<HashMap<String, (u32, chrono::DateTime<Local>)>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn hash_password(password: &str) -> Result<String, String> {
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| format!("Argon2 哈希失败: {}", e))
}

pub fn verify_password(password: &str, stored_hash: &str, salt: &str) -> (bool, bool) {
    if stored_hash.starts_with("$argon2") {
        if let Ok(parsed_hash) = PasswordHash::new(stored_hash) {
            let valid = Argon2::default()
                .verify_password(password.as_bytes(), &parsed_hash)
                .is_ok();
            return (valid, false);
        }
        (false, false)
    } else {
        // 向后兼容平滑兼容旧版本 SHA-256 哈希
        let mut hasher = sha2::Sha256::new();
        sha2::Digest::update(&mut hasher, password.as_bytes());
        sha2::Digest::update(&mut hasher, b":nexdns_salt:");
        sha2::Digest::update(&mut hasher, salt.as_bytes());
        let legacy_hex = format!("{:x}", sha2::Digest::finalize(hasher));
        if legacy_hex == stored_hash {
            (true, true) // 匹配成功，标记需自动平滑升级为 Argon2
        } else if salt.is_empty() && password == stored_hash {
            (true, true) // 支持配置文件中手写明文密码并自动升级为 Argon2
        } else {
            (false, false)
        }
    }
}

pub struct AuthManager;

impl AuthManager {
    pub fn cleanup_expired_sessions() {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let mut w = SESSION_CACHE.write();
        w.retain(|_, (_, exp)| exp.as_str() > now.as_str());
    }

    pub fn check_rate_limit(client_ip: &str) -> Result<(), String> {
        let now = Local::now();
        let mut w = LOGIN_ATTEMPTS.write();
        w.retain(|_, (_, time)| now.signed_duration_since(*time).num_seconds() < 300);
        if let Some((count, last_time)) = w.get_mut(client_ip) {
            let elapsed = now.signed_duration_since(*last_time).num_seconds();
            if *count >= 5 && elapsed < 60 {
                return Err(format!("登录尝试过于频繁，IP已被临时保护，请等待 {} 秒后再试", 60 - elapsed));
            }
            if elapsed >= 60 {
                *count = 0;
            }
        }
        Ok(())
    }

    pub fn record_login_failure(client_ip: &str) {
        let now = Local::now();
        let mut w = LOGIN_ATTEMPTS.write();
        let entry = w.entry(client_ip.to_string()).or_insert((0, now));
        entry.0 += 1;
        entry.1 = now;
    }

    pub fn reset_login_attempts(client_ip: &str) {
        LOGIN_ATTEMPTS.write().remove(client_ip);
    }

    pub fn authenticate(
        auth_cfg: &mut AuthConfig,
        username: &str,
        password: &str,
    ) -> Result<Option<(String, String, bool)>, String> {
        Self::cleanup_expired_sessions();
        if username.trim() != auth_cfg.username.trim() {
            return Ok(None);
        }

        let (valid, should_upgrade) = verify_password(password, &auth_cfg.password, "");
        if valid {
            let mut upgraded_flag = false;
            if should_upgrade {
                if let Ok(upgraded) = hash_password(password) {
                    auth_cfg.password = upgraded;
                    upgraded_flag = true;
                    tracing::info!("用户 [{}] 的密码哈希已自动平滑升级至 Argon2id 安全标准", username);
                }
            }
            let token = Uuid::new_v4().to_string();
            let expires_at = (Local::now() + Duration::days(7)).format("%Y-%m-%d %H:%M:%S").to_string();
            SESSION_CACHE
                .write()
                .insert(token.clone(), (auth_cfg.username.clone(), expires_at.clone()));
            Ok(Some((token, expires_at, upgraded_flag)))
        } else {
            Ok(None)
        }
    }

    pub fn validate_token(token: &str) -> Option<String> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let r = SESSION_CACHE.read();
        if let Some((user, exp)) = r.get(token) {
            if *exp > now {
                return Some(user.clone());
            }
        }
        None
    }

    pub fn logout(token: &str) {
        SESSION_CACHE.write().remove(token);
    }

    pub fn change_password(
        auth_cfg: &mut AuthConfig,
        new_username: Option<&str>,
        old_pass: &str,
        new_pass: &str,
    ) -> Result<(), String> {
        if new_pass.len() < 6 {
            return Err("新密码长度不能少于 6 位".to_string());
        }

        let (valid, _) = verify_password(old_pass, &auth_cfg.password, "");
        if !valid {
            return Err("原密码错误".to_string());
        }

        if let Some(nu) = new_username {
            let trimmed = nu.trim();
            if !trimmed.is_empty() {
                auth_cfg.username = trimmed.to_string();
            }
        }

        let new_hash = hash_password(new_pass)?;
        auth_cfg.password = new_hash;

        SESSION_CACHE.write().clear();

        Ok(())
    }
}
