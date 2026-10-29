use chrono::{Duration, Utc};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hasher.update(b":nexdns_salt:");
    hasher.update(salt.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
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
            let salt = Uuid::new_v4().to_string();
            let hash = hash_password("admin1234", &salt);
            conn.execute(
                "INSERT INTO users (username, password_hash, salt, created_at) VALUES (?1, ?2, ?3, ?4)",
                params!["admin", hash, salt, Utc::now().to_rfc3339()],
            )?;
            tracing::info!("创建默认管理员账户: admin / admin1234 (请在设置中及时修改密码)");
        }

        Ok(())
    }

    pub fn authenticate(conn: &Connection, username: &str, password: &str) -> Result<Option<(String, String)>, String> {
        let _ = Self::init_tables(conn);
        let mut stmt = conn
            .prepare("SELECT password_hash, salt FROM users WHERE username = ?1")
            .map_err(|e| e.to_string())?;

        let res = stmt.query_row(params![username], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        });

        match res {
            Ok((stored_hash, salt)) => {
                let input_hash = hash_password(password, &salt);
                if input_hash == stored_hash {
                    let token = Uuid::new_v4().to_string();
                    let expires_at = (Utc::now() + Duration::days(7)).to_rfc3339();
                    conn.execute(
                        "INSERT INTO sessions (token, username, expires_at) VALUES (?1, ?2, ?3)",
                        params![token, username, expires_at],
                    ).map_err(|e| e.to_string())?;
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
        let now = Utc::now().to_rfc3339();
        conn.query_row(
            "SELECT username FROM sessions WHERE token = ?1 AND expires_at > ?2",
            params![token, now],
            |row| row.get(0),
        ).ok()
    }

    pub fn logout(conn: &Connection, token: &str) -> rusqlite::Result<()> {
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

        let old_hash = hash_password(old_pass, &salt);
        if old_hash != stored_hash {
            return Err("原密码错误".to_string());
        }

        let new_salt = Uuid::new_v4().to_string();
        let new_hash = hash_password(new_pass, &new_salt);

        conn.execute(
            "UPDATE users SET password_hash = ?1, salt = ?2 WHERE username = ?3",
            params![new_hash, new_salt, username],
        ).map_err(|e| e.to_string())?;

        // Clear existing sessions for this user
        conn.execute("DELETE FROM sessions WHERE username = ?1", params![username])
            .map_err(|e| e.to_string())?;

        Ok(())
    }
}
