//! API key storage.
//!
//! Keys go to the operating system's credential store (Windows Credential
//! Manager on Windows). If that store is unavailable, the caller falls back to
//! the local database and tells the user so. Keys are never sent to the UI.

#[cfg(test)]
use std::collections::HashMap;
#[cfg(test)]
use std::sync::Mutex;

const SERVICE: &str = "com.examops.app";

pub trait SecretStore: Send + Sync {
    fn set(&self, name: &str, secret: &str) -> Result<(), String>;
    fn get(&self, name: &str) -> Result<Option<String>, String>;
    fn delete(&self, name: &str) -> Result<(), String>;
    fn label(&self) -> &'static str;
}

/// The platform credential store via the `keyring` crate.
pub struct KeyringStore;

impl SecretStore for KeyringStore {
    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        keyring::Entry::new(SERVICE, name)
            .and_then(|entry| entry.set_password(secret))
            .map_err(|e| e.to_string())
    }

    fn get(&self, name: &str) -> Result<Option<String>, String> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
        match entry.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        let entry = keyring::Entry::new(SERVICE, name).map_err(|e| e.to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn label(&self) -> &'static str {
        if cfg!(windows) {
            "Windows 凭据管理器"
        } else if cfg!(target_os = "macos") {
            "macOS 钥匙串"
        } else {
            "系统密钥环"
        }
    }
}

/// In-memory store for tests.
#[cfg(test)]
#[derive(Default)]
pub struct MemoryStore {
    entries: Mutex<HashMap<String, String>>,
    fail: bool,
}

#[cfg(test)]
impl MemoryStore {
    /// A store that behaves like an unavailable credential manager.
    pub fn failing() -> Self {
        MemoryStore { fail: true, ..Default::default() }
    }
}

#[cfg(test)]
impl SecretStore for MemoryStore {
    fn set(&self, name: &str, secret: &str) -> Result<(), String> {
        if self.fail {
            return Err("store unavailable".into());
        }
        self.entries.lock().unwrap().insert(name.to_string(), secret.to_string());
        Ok(())
    }

    fn get(&self, name: &str) -> Result<Option<String>, String> {
        if self.fail {
            return Err("store unavailable".into());
        }
        Ok(self.entries.lock().unwrap().get(name).cloned())
    }

    fn delete(&self, name: &str) -> Result<(), String> {
        self.entries.lock().unwrap().remove(name);
        Ok(())
    }

    fn label(&self) -> &'static str {
        "测试存储"
    }
}

/// `sk-abcd…wxyz` style hint so users can tell keys apart without exposing them.
pub fn key_hint(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() <= 8 {
        return "••••".into();
    }
    let head: String = chars[..4].iter().collect();
    let tail: String = chars[chars.len() - 4..].iter().collect();
    format!("{head}…{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_never_reveal_the_key() {
        assert_eq!(key_hint("sk-1234567890abcdef"), "sk-1…cdef");
        assert_eq!(key_hint("short"), "••••");
    }
}
