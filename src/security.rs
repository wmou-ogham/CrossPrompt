use std::{
    collections::BTreeMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
    Aes256Gcm, Nonce,
};
use anyhow::{bail, Context, Result};
use axum::http::HeaderMap;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use tokio::net::lookup_host;
use url::{Host, Url};

use crate::models::NotificationConfig;

pub fn new_secret() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn digest(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

pub fn keyed_digest(key: &str, value: &str) -> Vec<u8> {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.as_bytes())
        .expect("HMAC accepts keys of any size");
    mac.update(value.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub fn salted_digest(salt: &str, value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    URL_SAFE_NO_PAD.encode(hasher.finalize())
}

pub fn encrypt_config(key: &[u8; 32], config: &NotificationConfig) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
    let mut nonce = [0u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let plaintext = serde_json::to_vec(config)?;
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce), plaintext.as_ref())
        .map_err(|_| anyhow::anyhow!("configuration encryption failed"))?;
    let mut output = nonce.to_vec();
    output.extend(ciphertext);
    Ok(output)
}

pub fn decrypt_config(key: &[u8; 32], value: &[u8]) -> Result<NotificationConfig> {
    if value.len() < 13 {
        bail!("encrypted configuration is invalid");
    }
    let (nonce, ciphertext) = value.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key).expect("32-byte key");
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| anyhow::anyhow!("configuration decryption failed"))?;
    Ok(serde_json::from_slice(&plaintext)?)
}

pub fn mask_url(raw: &str) -> String {
    match Url::parse(raw) {
        Ok(url) => format!("{}://{}/••••••••", url.scheme(), url.host_str().unwrap_or("hidden")),
        Err(_) => "••••••••".into(),
    }
}

pub fn client_ip(headers: &HeaderMap, peer: SocketAddr, trust_proxy: bool) -> IpAddr {
    if trust_proxy {
        if let Some(value) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
            if let Some(ip) = value.split(',').next().and_then(|v| v.trim().parse().ok()) {
                return ip;
            }
        }
    }
    peer.ip()
}

pub async fn validate_webhook_url(raw: &str) -> Result<(Url, Vec<SocketAddr>)> {
    let url = Url::parse(raw).context("invalid webhook URL")?;
    if url.scheme() != "https" {
        bail!("webhook URL must use https");
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("credentials in webhook URLs are not allowed");
    }
    if url.port_or_known_default() != Some(443) {
        bail!("webhook URL must use port 443");
    }
    let host = url.host().context("webhook URL needs a host")?;
    let port = 443;
    let addresses = match host {
        Host::Ipv4(ip) => vec![SocketAddr::new(IpAddr::V4(ip), port)],
        Host::Ipv6(ip) => vec![SocketAddr::new(IpAddr::V6(ip), port)],
        Host::Domain(domain) => lookup_host((domain, port)).await?.collect::<Vec<_>>(),
    };
    if addresses.is_empty() || addresses.iter().any(|address| forbidden_ip(address.ip())) {
        bail!("webhook host resolves to a forbidden network");
    }
    Ok((url, addresses))
}

pub fn validate_headers(headers: &BTreeMap<String, String>) -> Result<()> {
    if headers.len() > 12 {
        bail!("at most 12 webhook headers are allowed");
    }
    for (name, value) in headers {
        let lower = name.to_ascii_lowercase();
        if !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || matches!(lower.as_str(), "host" | "content-length" | "connection" | "transfer-encoding")
            || value.contains(['\r', '\n'])
            || value.len() > 1024
        {
            bail!("invalid webhook header");
        }
    }
    Ok(())
}

pub fn forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => forbidden_v4(ip),
        IpAddr::V6(ip) => forbidden_v6(ip),
    }
}

fn forbidden_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_broadcast()
        || ip.is_unspecified()
        || a == 0
        || a >= 224
        || a == 100 && (64..=127).contains(&b)
        || a == 192 && b == 0 && c == 0
        || a == 192 && b == 0 && c == 2
        || a == 198 && (b == 18 || b == 19)
        || a == 198 && b == 51 && c == 100
        || a == 203 && b == 0 && c == 113
    // Includes 169.254.169.254 through link-local.
}

fn forbidden_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00 // unique local
        || (segments[0] & 0xffc0) == 0xfe80 // link local
        || (segments[0] == 0x2001 && segments[1] == 0x0db8) // documentation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_high_entropy_and_url_safe() {
        let a = new_secret();
        let b = new_secret();
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
        assert!(!a.contains(['/', '+', '=']));
    }

    #[test]
    fn rejects_private_networks() {
        for ip in ["127.0.0.1", "10.2.3.4", "169.254.169.254", "192.0.2.1", "::1", "fc00::1"] {
            assert!(forbidden_ip(ip.parse().unwrap()), "{ip}");
        }
        assert!(!forbidden_ip("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn encrypted_notification_round_trip() {
        let key = [9u8; 32];
        let config = NotificationConfig { url: "https://example.com/a".into(), headers: BTreeMap::new() };
        let encrypted = encrypt_config(&key, &config).unwrap();
        assert!(!String::from_utf8_lossy(&encrypted).contains("example.com"));
        assert_eq!(decrypt_config(&key, &encrypted).unwrap().url, config.url);
    }

    #[test]
    fn keyed_digests_are_peppered() {
        assert_eq!(keyed_digest("a", "token"), keyed_digest("a", "token"));
        assert_ne!(keyed_digest("a", "token"), keyed_digest("b", "token"));
    }
}
