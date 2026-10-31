use chrono::{Duration, Local};
use rusqlite::{params, Connection};
use sha2::Digest;
use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::LazyLock;
use uuid::Uuid;

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
        } else {
            (false, false)
        }
    }
}

pub struct AuthManager;

impl AuthManager {
    pub fn init_tables(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS users (
                username TEXT PRIMARY KEY,
                password_hash TEXT NOT NULL,
                salt TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS sessions (
                token TEXT PRIMARY KEY,
                username TEXT NOT NULL,
                expires_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);"
        )?;

        // Check if admin user exists; if not, create default admin / admin1234
        let user_count: i64 = conn.query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
        if user_count == 0 {
            let hash = hash_password("admin1234").map_err(|e| {
                rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)))
            })?;
            conn.execute(
                "INSERT INTO users (username, password_hash, salt, created_at) VALUES (?1, ?2, ?3, ?4)",
                params!["admin", hash, "", Local::now().format("%Y-%m-%d %H:%M:%S").to_string()],
            )?;
            tracing::info!("创建默认管理员账户: admin / admin1234 (采用 Argon2id 高安全性哈希，请在设置中及时修改密码)");
        }

        let _ = Self::cleanup_expired_sessions(conn);

        Ok(())
    }

    pub fn cleanup_expired_sessions(conn: &Connection) -> rusqlite::Result<()> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        conn.execute("DELETE FROM sessions WHERE expires_at <= ?1", params![now])?;
        let mut w = SESSION_CACHE.write();
        w.retain(|_, (_, exp)| exp.as_str() > now.as_str());
        Ok(())
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

    pub fn authenticate(conn: &Connection, username: &str, password: &str) -> Result<Option<(String, String)>, String> {
        let _ = Self::init_tables(conn);
        let _ = Self::cleanup_expired_sessions(conn);
        let mut stmt = conn
            .prepare("SELECT password_hash, salt FROM users WHERE username = ?1")
            .map_err(|e| e.to_string())?;

        let res = stmt.query_row(params![username], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });

        match res {
            Ok((stored_hash, salt)) => {
                let (valid, should_upgrade) = verify_password(password, &stored_hash, &salt);
                if valid {
                    if should_upgrade {
                        if let Ok(upgraded) = hash_password(password) {
                            let _ = conn.execute(
                                "UPDATE users SET password_hash = ?1, salt = '' WHERE username = ?2",
                                params![upgraded, username],
                            );
                            tracing::info!("用户 [{}] 的密码哈希已自动平滑升级至 Argon2id 安全标准", username);
                        }
                    }
                    let token = Uuid::new_v4().to_string();
                    let expires_at = (Local::now() + Duration::days(7)).format("%Y-%m-%d %H:%M:%S").to_string();
                    conn.execute(
                        "INSERT INTO sessions (token, username, expires_at) VALUES (?1, ?2, ?3)",
                        params![token, username, expires_at],
                    ).map_err(|e| e.to_string())?;
                    SESSION_CACHE.write().insert(token.clone(), (username.to_string(), expires_at.clone()));
                    Ok(Some((token, expires_at)))
                } else {
                    Ok(None)
                }
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn validate_token(conn: &Connection, token: &str) -> Option<String> {
        let _ = Self::init_tables(conn);
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        {
            let r = SESSION_CACHE.read();
            if let Some((user, exp)) = r.get(token) {
                if *exp > now {
                    return Some(user.clone());
                }
            }
        }
        let res: Option<(String, String)> = conn.query_row(
            "SELECT username, expires_at FROM sessions WHERE token = ?1 AND expires_at > ?2",
            params![token, now],
            |row| Ok((row.get(0)?, row.get(1)?)),
        ).ok();
        if let Some((u, exp)) = res {
            SESSION_CACHE.write().insert(token.to_string(), (u.clone(), exp));
            Some(u)
        } else {
            None
        }
    }

    pub fn logout(conn: &Connection, token: &str) -> rusqlite::Result<()> {
        SESSION_CACHE.write().remove(token);
        conn.execute("DELETE FROM sessions WHERE token = ?1", params![token])?;
        Ok(())
    }

    pub fn change_password(conn: &Connection, username: &str, old_pass: &str, new_pass: &str) -> Result<(), String> {
        let _ = Self::init_tables(conn);
        if new_pass.len() < 6 {
            return Err("新密码长度不能少于 6 位".to_string());
        }

        let mut stmt = conn
            .prepare("SELECT password_hash, salt FROM users WHERE username = ?1")
            .map_err(|e| e.to_string())?;

        let (stored_hash, salt) = stmt.query_row(params![username], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        }).map_err(|_| "用户不存在".to_string())?;

        let (valid, _) = verify_password(old_pass, &stored_hash, &salt);
        if !valid {
            return Err("原密码错误".to_string());
        }

        let new_hash = hash_password(new_pass)?;

        conn.execute(
            "UPDATE users SET password_hash = ?1, salt = '' WHERE username = ?2",
            params![new_hash, username],
        ).map_err(|e| e.to_string())?;

        // Clear existing sessions for this user
        SESSION_CACHE.write().retain(|_, (u, _)| u != username);
        conn.execute("DELETE FROM sessions WHERE username = ?1", params![username])
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
