use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::{bail, Context, Result};
use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher};
use base64::{engine::general_purpose::STANDARD, Engine};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub database_path: PathBuf,
    pub frontend_dir: PathBuf,
    pub public_base_url: String,
    pub app_env: String,
    pub admin_username: String,
    pub admin_password_hash: String,
    pub session_secret: String,
    pub master_key: [u8; 32],
    pub ip_hash_salt: String,
    pub turnstile_secret_key: Option<String>,
    pub turnstile_site_key: Option<String>,
    pub cookie_secure: bool,
    pub trust_proxy: bool,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenvy::dotenv().ok();
        let app_env = value("CROSSPROMPT_ENV", "development");
        let production = app_env == "production";
        let database_url = value("CROSSPROMPT_DATABASE_URL", "sqlite:///data/crossprompt.db");
        let database_path = sqlite_path(&database_url)?;

        let admin_username = env::var("CROSSPROMPT_ADMIN_USERNAME")
            .unwrap_or_else(|_| "admin".into());
        let admin_password_hash = env::var("CROSSPROMPT_ADMIN_PASSWORD_HASH")
            .unwrap_or_else(|_| development_password_hash());
        let session_secret = env::var("CROSSPROMPT_SESSION_SECRET")
            .unwrap_or_else(|_| "development-session-secret-change-me".into());
        let ip_hash_salt = env::var("CROSSPROMPT_IP_HASH_SALT")
            .unwrap_or_else(|_| "development-ip-salt-change-me".into());
        let master_key = match env::var("CROSSPROMPT_MASTER_KEY") {
            Ok(raw) => decode_master_key(&raw)?,
            Err(_) if !production => {
                let digest = Sha256::digest(b"crossprompt-development-master-key");
                digest.into()
            }
            Err(_) => bail!("CROSSPROMPT_MASTER_KEY is required in production"),
        };

        if production {
            for key in [
                "CROSSPROMPT_ADMIN_USERNAME",
                "CROSSPROMPT_ADMIN_PASSWORD_HASH",
                "CROSSPROMPT_SESSION_SECRET",
                "CROSSPROMPT_MASTER_KEY",
                "CROSSPROMPT_IP_HASH_SALT",
            ] {
                if env::var(key).is_err() {
                    bail!("{key} is required in production");
                }
            }
            if session_secret.len() < 32 || ip_hash_salt.len() < 24 {
                bail!("production session secret and IP salt are too short");
            }
        }
        if admin_username.trim().is_empty() {
            bail!("CROSSPROMPT_ADMIN_USERNAME cannot be empty");
        }
        PasswordHash::new(&admin_password_hash)
            .map_err(|error| anyhow::anyhow!("invalid CROSSPROMPT_ADMIN_PASSWORD_HASH: {error}"))?;

        Ok(Self {
            bind_addr: value("CROSSPROMPT_BIND", "0.0.0.0:8080")
                .parse()
                .context("invalid CROSSPROMPT_BIND")?,
            database_url,
            database_path,
            frontend_dir: PathBuf::from(value("CROSSPROMPT_FRONTEND_DIR", "/app/static")),
            public_base_url: value("CROSSPROMPT_PUBLIC_BASE_URL", "http://localhost:8080")
                .trim_end_matches('/')
                .to_owned(),
            app_env,
            admin_username,
            admin_password_hash,
            session_secret,
            master_key,
            ip_hash_salt,
            turnstile_secret_key: env::var("CROSSPROMPT_TURNSTILE_SECRET_KEY").ok(),
            turnstile_site_key: env::var("CROSSPROMPT_TURNSTILE_SITE_KEY").ok(),
            cookie_secure: bool_value("CROSSPROMPT_COOKIE_SECURE", production),
            trust_proxy: bool_value("CROSSPROMPT_TRUST_PROXY", false),
        })
    }
}

fn value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_owned())
}

fn bool_value(key: &str, default: bool) -> bool {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn sqlite_path(url: &str) -> Result<PathBuf> {
    let raw = url
        .strip_prefix("sqlite://")
        .context("database URL must start with sqlite://")?;
    Ok(PathBuf::from(raw.split('?').next().unwrap_or(raw)))
}

fn decode_master_key(raw: &str) -> Result<[u8; 32]> {
    let bytes = STANDARD
        .decode(raw)
        .context("CROSSPROMPT_MASTER_KEY must be base64")?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("CROSSPROMPT_MASTER_KEY must decode to 32 bytes"))
}

fn development_password_hash() -> String {
    // Password: admin. Production refuses to use this implicit fallback.
    let salt = SaltString::encode_b64(b"crossprompt-dev").expect("valid static salt");
    Argon2::default()
        .hash_password(b"admin", &salt)
        .expect("valid development password hash")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use argon2::PasswordVerifier;

    #[test]
    fn development_admin_password_is_valid() {
        let encoded = development_password_hash();
        let hash = PasswordHash::new(&encoded).unwrap();
        assert!(Argon2::default().verify_password(b"admin", &hash).is_ok());
    }
}
