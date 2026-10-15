//! Vault files: share one config as a password-locked, expiring blob.
//!
//! Our own format (NOT ZedSecure .zsx — different crypto): magic +
//! Argon2id(password, salt) → AES-256-GCM(JSON {payload, expiry}).
//! Wrong password and expiry are told apart so the UI can say which.
use aes_gcm::{aead::{Aead, KeyInit, OsRng}, Aes256Gcm, Nonce};
use argon2::Argon2;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

const MAGIC: &[u8] = b"AETHER-VAULT1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultPayload {
    pub name: String,
    pub engine: String,
    pub config_json: String,
    /// Unix seconds, 0 = never expires.
    pub expiry: u64,
}

fn derive_key(password: &str, salt: &[u8]) -> Result<[u8; 32], String> {
    if password.len() < 4 {
        return Err("password too short".into());
    }
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), salt, &mut key)
        .map_err(|e| format!("kdf: {e}"))?;
    Ok(key)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Seal a config JSON. Returns base64 blob for a `.vault` file.
#[tauri::command]
pub fn vault_seal(config_json: String, name: String, password: String, days_valid: u64) -> Result<String, String> {
    if serde_json::from_str::<serde_json::Value>(&config_json).is_err() {
        return Err("not json".into());
    }
    let mut salt = [0u8; SALT_LEN];
    getrandom_fill(&mut salt)?;
    let key = derive_key(&password, &salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher: {e}"))?;
    let mut nonce_bytes = [0u8; NONCE_LEN];
    getrandom_fill(&mut nonce_bytes)?;
    let payload = serde_json::to_vec(&VaultPayload {
        name,
        engine: String::new(),
        config_json,
        expiry: if days_valid == 0 { 0 } else { now_secs().saturating_add(days_valid * 86400) },
    })
    .map_err(|e| format!("json: {e}"))?;
    let ct = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), payload.as_ref())
        .map_err(|e| format!("seal: {e}"))?;
    let mut blob = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ct.len());
    blob.extend_from_slice(MAGIC);
    blob.extend_from_slice(&salt);
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ct);
    Ok(base64::engine::general_purpose::STANDARD.encode(blob))
}

#[derive(Debug, Clone, Serialize)]
pub struct VaultOpened {
    pub name: String,
    pub config_json: String,
    pub expired: bool,
}

#[tauri::command]
pub fn vault_open(blob_b64: String, password: String) -> Result<VaultOpened, String> {
    let blob = base64::engine::general_purpose::STANDARD
        .decode(blob_b64.trim())
        .map_err(|_| "not a vault file".to_string())?;
    if blob.len() < MAGIC.len() + SALT_LEN + NONCE_LEN + 1 || &blob[..MAGIC.len()] != MAGIC {
        return Err("not a vault file".into());
    }
    let salt = &blob[MAGIC.len()..MAGIC.len() + SALT_LEN];
    let nonce = &blob[MAGIC.len() + SALT_LEN..MAGIC.len() + SALT_LEN + NONCE_LEN];
    let ct = &blob[MAGIC.len() + SALT_LEN + NONCE_LEN..];
    let key = derive_key(&password, salt)?;
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| format!("cipher: {e}"))?;
    let pt = cipher
        .decrypt(Nonce::from_slice(nonce), ct)
        .map_err(|_| "wrong password".to_string())?;
    let payload: VaultPayload =
        serde_json::from_slice(&pt).map_err(|_| "corrupt vault".to_string())?;
    let expired = payload.expiry != 0 && now_secs() > payload.expiry;
    Ok(VaultOpened {
        name: payload.name,
        config_json: payload.config_json,
        expired,
    })
}

fn getrandom_fill(buf: &mut [u8]) -> Result<(), String> {
    #[cfg(windows)]
    {
        #[link(name = "bcrypt")]
        extern "system" {
            fn BCryptGenRandom(
                halgorithm: *mut std::ffi::c_void,
                pbbuffer: *mut u8,
                cbbuffer: u32,
                dwflags: u32,
            ) -> i32;
        }
        // BCRYPT_USE_SYSTEM_PREFERRED_RNG with a null handle.
        let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), buf.as_mut_ptr(), buf.len() as u32, 2) };
        if status == 0 {
            Ok(())
        } else {
            Err(format!("rng: {status}"))
        }
    }
    #[cfg(not(windows))]
    {
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(buf))
            .map_err(|e| format!("rng: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_open_roundtrip() {
        let blob = vault_seal(r#"{"a":1}"#.into(), "n".into(), "correct horse".into(), 7).unwrap();
        let opened = vault_open(blob, "correct horse".into()).unwrap();
        assert!(!opened.expired);
        assert_eq!(opened.config_json, r#"{"a":1}"#);
    }

    #[test]
    fn wrong_password_and_expiry() {
        let blob = vault_seal(r#"{}"#.into(), "n".into(), "right".into(), 0).unwrap();
        assert!(vault_open(blob.clone(), "wrong".into()).is_err());
        // Expired payload opens but flags.
        let old = {
            let mut salt = [0u8; SALT_LEN];
            getrandom_fill(&mut salt).unwrap();
            let key = derive_key("pw1234", &salt).unwrap();
            let cipher = Aes256Gcm::new_from_slice(&key).unwrap();
            let mut nonce = [0u8; NONCE_LEN];
            getrandom_fill(&mut nonce).unwrap();
            let pt = serde_json::to_vec(&VaultPayload {
                name: "n".into(),
                engine: String::new(),
                config_json: "{}".into(),
                expiry: 1,
            })
            .unwrap();
            let ct = cipher.encrypt(Nonce::from_slice(&nonce), pt.as_ref()).unwrap();
            let mut blob = Vec::new();
            blob.extend_from_slice(MAGIC);
            blob.extend_from_slice(&salt);
            blob.extend_from_slice(&nonce);
            blob.extend_from_slice(&ct);
            base64::engine::general_purpose::STANDARD.encode(blob)
        };
        let opened = vault_open(old, "pw1234".into()).unwrap();
        assert!(opened.expired);
    }

    #[test]
    fn rejects_junk_and_short_password() {
        assert!(vault_open("aGVsbG8=".into(), "password".into()).is_err());
        assert!(vault_seal("{}".into(), "n".into(), "abc".into(), 0).is_err());
        assert!(vault_seal("not json".into(), "n".into(), "password".into(), 0).is_err());
    }
}
