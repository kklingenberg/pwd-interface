//! Exposes functions that operate on secure text tokens.

use base64::{engine::general_purpose, Engine as _};
use rand::{thread_rng, RngCore};
use sha2::{Digest, Sha512};
use std::time::SystemTime;

/// Generate a random token of at least the requested size, in bytes
pub fn make(size: usize) -> String {
    let fitted = if size % 3 == 0 {
        size
    } else {
        size + 3 - (size % 3)
    };
    let mut data: Vec<u8> = vec![0; fitted];
    thread_rng().fill_bytes(&mut data);
    general_purpose::URL_SAFE_NO_PAD.encode(data)
}

/// Obfuscate the given text with the given salt
pub fn salt_text(text: &str, given_salt: Option<&str>) -> String {
    let salt = if let Some(s) = given_salt {
        String::from(s)
    } else {
        make(9)
    };
    let mut hasher = Sha512::new();
    hasher.update(format!("{}{}", text, salt).as_bytes());
    format!("{}${:x}", salt, hasher.finalize())
}

/// Obfuscate the given text with a time-based salt
pub fn salt_timed(text: &str) -> Option<String> {
    let interval = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() / 30)
        .ok()?;
    let salt = format!("{:x}", interval);
    Some(salt_text(text, Some(&salt)))
}

/// Verify a salted token, assuming the salt is generated from the
/// current count of 30-second intervals from unix timestamp
/// zero. Tollerate a single shift forwards or backwards in time for
/// said salt.
pub fn verify_timed(salted: &str, token: &str) -> bool {
    salted
        .split('$')
        .next()
        .and_then(|salt| {
            let interval = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs() / 30)
                .ok()?;
            let used_salt = [interval, interval - 1, interval + 1]
                .iter()
                .map(|t| format!("{:x}", t))
                .find(|valid_salt| salt == valid_salt)?;
            Some(salted == salt_text(token, Some(&used_salt)))
        })
        .unwrap_or_default()
}
