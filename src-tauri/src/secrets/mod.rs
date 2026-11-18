use crate::profiles::{Profile, ProtocolConfig};
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use rand::RngCore;
use std::fs;
use std::path::Path;
use std::sync::{Arc, RwLock};

pub const ENC_PREFIX: &str = "enc:v1:";

#[derive(Clone, Default)]
pub struct SecretVault {
    master_key: Arc<RwLock<Option<[u8; 32]>>>,
}

impl SecretVault {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_key(&self, key: [u8; 32]) {
        if let Ok(mut lock) = self.master_key.write() {
            *lock = Some(key);
        }
    }

    pub fn has_key(&self) -> bool {
        self.master_key.read().map(|g| g.is_some()).unwrap_or(false)
    }

    /// Initializes the vault for a config directory.
    /// On desktop (non-test), uses the OS Keyring (Credential Manager / Keychain / Secret Service)
    /// and falls back to a 0600 keyfile only if the OS Keyring is unavailable.
    pub fn init_for_dir(&self, dir: &Path) {
        #[cfg(all(not(test), not(any(target_os = "android", target_os = "ios"))))]
        {
            if let Some(key) = Self::load_or_create_keyring_key() {
                self.set_key(key);
                return;
            }
        }

        // On Android before AppHandle is bound, or in tests / headless Linux without Secret Service,
        // use a 0600 keyfile in the app config directory.
        #[cfg(not(target_os = "android"))]
        if let Some(key) = Self::load_or_create_keyfile(&dir.join(".master.key")) {
            self.set_key(key);
        }
        #[cfg(target_os = "android")]
        {
            let _ = dir;
        }
    }

    #[cfg(all(feature = "gui", target_os = "android"))]
    pub fn init_android(&self, handle: &tauri::AppHandle, fallback_dir: &Path) {
        use tauri_plugin_rustybox_vpn::RustyBoxVpnExt;
        match handle.rustybox_vpn().get_or_create_master_key() {
            Ok(b64) => {
                if let Ok(bytes) = B64.decode(b64.trim()) {
                    if bytes.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&bytes);
                        self.set_key(key);
                        return;
                    }
                }
                tracing::warn!("Android Keystore returned invalid master key length");
            }
            Err(e) => {
                tracing::warn!("Failed to get master key from Android Keystore: {}", e);
            }
        }
        if let Some(key) = Self::load_or_create_keyfile(&fallback_dir.join(".master.key")) {
            self.set_key(key);
        }
    }

    #[cfg(all(not(test), not(any(target_os = "android", target_os = "ios"))))]
    fn load_or_create_keyring_key() -> Option<[u8; 32]> {
        let entry = match keyring::Entry::new("com.rustybox.app", "master-key") {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("OS keyring unavailable ({}), falling back to keyfile", e);
                return None;
            }
        };

        match entry.get_password() {
            Ok(b64) => {
                if let Ok(bytes) = B64.decode(b64.trim()) {
                    if bytes.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&bytes);
                        return Some(key);
                    }
                }
            }
            Err(keyring::Error::NoEntry) => {}
            Err(e) => {
                tracing::warn!("OS keyring read failed ({}), falling back to keyfile", e);
                return None;
            }
        }

        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let b64 = B64.encode(key);
        match entry.set_password(&b64) {
            Ok(()) => Some(key),
            Err(e) => {
                tracing::warn!("OS keyring write failed ({}), falling back to keyfile", e);
                None
            }
        }
    }

    fn load_or_create_keyfile(path: &Path) -> Option<[u8; 32]> {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(bytes) = B64.decode(content.trim()) {
                    if bytes.len() == 32 {
                        let mut key = [0u8; 32];
                        key.copy_from_slice(&bytes);
                        return Some(key);
                    }
                }
            }
        }
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let b64 = B64.encode(key);
        if write_secure_file(path, b64.as_bytes()).is_ok() {
            Some(key)
        } else {
            None
        }
    }

    pub fn encrypt_str(&self, plaintext: &str) -> String {
        if plaintext.is_empty() || plaintext.starts_with(ENC_PREFIX) {
            return plaintext.to_string();
        }
        let key = match self.master_key.read().ok().and_then(|g| *g) {
            Some(k) => k,
            None => return plaintext.to_string(),
        };

        let cipher = Aes256Gcm::new_from_slice(&key).expect("32-byte key");
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        match cipher.encrypt(nonce, plaintext.as_bytes()) {
            Ok(ct) => {
                let mut combined = Vec::with_capacity(12 + ct.len());
                combined.extend_from_slice(&nonce_bytes);
                combined.extend_from_slice(&ct);
                format!("{}{}", ENC_PREFIX, B64.encode(&combined))
            }
            Err(_) => plaintext.to_string(),
        }
    }

    pub fn decrypt_str(&self, value: &str) -> String {
        if !value.starts_with(ENC_PREFIX) {
            return value.to_string();
        }
        let key = match self.master_key.read().ok().and_then(|g| *g) {
            Some(k) => k,
            None => return value.to_string(),
        };

        let b64 = &value[ENC_PREFIX.len()..];
        let combined = match B64.decode(b64.trim()) {
            Ok(b) => b,
            Err(_) => return value.to_string(),
        };
        if combined.len() <= 12 {
            return value.to_string();
        }

        let (nonce_bytes, ct) = combined.split_at(12);
        let cipher = Aes256Gcm::new_from_slice(&key).expect("32-byte key");
        let nonce = Nonce::from_slice(nonce_bytes);

        match cipher.decrypt(nonce, ct) {
            Ok(pt) => String::from_utf8(pt).unwrap_or_else(|_| value.to_string()),
            Err(_) => value.to_string(),
        }
    }

    pub fn encrypt_profile(&self, profile: &mut Profile) {
        match &mut profile.protocol {
            ProtocolConfig::Ssh(cfg) => {
                if let Some(ref mut pwd) = cfg.password {
                    *pwd = self.encrypt_str(pwd);
                }
                if let Some(ref mut pk) = cfg.private_key {
                    *pk = self.encrypt_str(pk);
                }
                if let Some(ref mut pass) = cfg.private_key_passphrase {
                    *pass = self.encrypt_str(pass);
                }
            }
            ProtocolConfig::Shadowsocks(cfg) => {
                cfg.password = self.encrypt_str(&cfg.password);
            }
            ProtocolConfig::Openvpn(cfg) => {
                if let Some(ref mut pwd) = cfg.password {
                    *pwd = self.encrypt_str(pwd);
                }
                if let Some(ref mut ck) = cfg.client_key {
                    *ck = self.encrypt_str(ck);
                }
            }
            ProtocolConfig::Vless(_) => {}
        }
    }

    pub fn decrypt_profile(&self, profile: &mut Profile) {
        match &mut profile.protocol {
            ProtocolConfig::Ssh(cfg) => {
                if let Some(ref mut pwd) = cfg.password {
                    *pwd = self.decrypt_str(pwd);
                }
                if let Some(ref mut pk) = cfg.private_key {
                    *pk = self.decrypt_str(pk);
                }
                if let Some(ref mut pass) = cfg.private_key_passphrase {
                    *pass = self.decrypt_str(pass);
                }
            }
            ProtocolConfig::Shadowsocks(cfg) => {
                cfg.password = self.decrypt_str(&cfg.password);
            }
            ProtocolConfig::Openvpn(cfg) => {
                if let Some(ref mut pwd) = cfg.password {
                    *pwd = self.decrypt_str(pwd);
                }
                if let Some(ref mut ck) = cfg.client_key {
                    *ck = self.decrypt_str(ck);
                }
            }
            ProtocolConfig::Vless(_) => {}
        }
    }
}

/// Writes a file with owner-only permissions (0600 on Unix).
pub fn write_secure_file(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(contents)?;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        Ok(())
    }

    #[cfg(not(unix))]
    {
        fs::write(path, contents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let vault = SecretVault::new();
        vault.set_key([42u8; 32]);

        let secret =
            "-----BEGIN OPENSSH PRIVATE KEY-----\nabc123\n-----END OPENSSH PRIVATE KEY-----";
        let enc = vault.encrypt_str(secret);
        assert!(enc.starts_with(ENC_PREFIX));
        assert_ne!(enc, secret);

        // Encrypting an already-encrypted string must be idempotent
        assert_eq!(vault.encrypt_str(&enc), enc);

        let dec = vault.decrypt_str(&enc);
        assert_eq!(dec, secret);

        // Legacy plaintext passes through decrypt_str unchanged
        assert_eq!(vault.decrypt_str("legacy-plain"), "legacy-plain");
    }
}
