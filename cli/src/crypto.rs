use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use hmac::{Hmac, Mac as HmacMac};
use rand::Rng;
use sha2::Sha256;

use crate::models::{BattleTeam, CaughtPokemon, PcStorage, Pokedex};

// Build-time generated secret — never exists in source code
// This defines `const BUILD_SECRET: [u8; 32]` and `const API_URL: &str`
include!(concat!(env!("OUT_DIR"), "/build_secret.rs"));

// Re-export API_URL as a pub function since the include! generates a private const
pub fn api_url() -> &'static str {
    API_URL
}

pub type HmacSha256 = Hmac<Sha256>;

// Domain separation constants — scattered across the binary to avoid simple extraction
pub const KDF_DOMAIN: &[u8] = b"catch-pokemon:kdf:v1";
pub const SIGN_DOMAIN: &[u8] = b"catch-pokemon:sign:v1";
pub const CHAIN_DOMAIN: &[u8] = b"catch-pokemon:chain:v1";
pub const ENCRYPTION_DOMAIN: &[u8] = b"catch-pokemon:encryption:v1";

/// Derive signing key from BUILD_SECRET only. No salt.
/// Same key on every machine with the same binary.
/// API uses the same BUILD_SECRET to verify.
pub fn derive_signing_key() -> Vec<u8> {
    let mut mac =
        <HmacSha256 as HmacMac>::new_from_slice(&BUILD_SECRET).expect("HMAC accepts any key length");
    mac.update(KDF_DOMAIN);
    let mut key = mac.finalize().into_bytes().to_vec();

    for round in 0u32..10_000 {
        let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&key)
            .expect("HMAC accepts any key length");
        mac.update(&round.to_le_bytes());
        mac.update(SIGN_DOMAIN);
        key = mac.finalize().into_bytes().to_vec();
    }

    key
}

/// Derive a 32-byte AES key from the signing key
pub fn derive_encryption_key() -> [u8; 32] {
    let signing_key = derive_signing_key();
    let mut mac = <HmacSha256 as HmacMac>::new_from_slice(&signing_key)
        .expect("HMAC accepts any key length");
    mac.update(ENCRYPTION_DOMAIN);
    let result = mac.finalize().into_bytes();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Encrypt PC storage: output = [12-byte nonce][AES-256-GCM ciphertext]
pub fn encrypt_storage(storage: &PcStorage) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key)?;

    let json = serde_json::to_string(storage)?;

    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    for b in nonce_bytes.iter_mut() {
        *b = rng.gen();
    }
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;

    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Decrypt PC storage from encrypted bytes
pub fn decrypt_storage(data: &[u8]) -> Option<PcStorage> {
    if data.len() < 13 {
        return None;
    }

    // If it starts with '{', it's unencrypted legacy JSON
    if data[0] == b'{' {
        return None;
    }

    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;

    let nonce = Nonce::from_slice(&data[..12]);
    let ciphertext = &data[12..];

    let plaintext = cipher.decrypt(nonce, ciphertext).ok()?;
    let json_str = String::from_utf8(plaintext).ok()?;
    serde_json::from_str(&json_str).ok()
}

pub fn encrypt_pokedex(dex: &Pokedex) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let json = serde_json::to_string(dex)?;
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    for b in nonce_bytes.iter_mut() {
        *b = rng.gen();
    }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub fn decrypt_pokedex(data: &[u8]) -> Option<Pokedex> {
    if data.len() < 13 || data[0] == b'{' {
        return None;
    }
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let nonce = Nonce::from_slice(&data[..12]);
    let plaintext = cipher.decrypt(nonce, &data[12..]).ok()?;
    let json_str = String::from_utf8(plaintext).ok()?;
    serde_json::from_str(&json_str).ok()
}

pub fn encrypt_battle_team(team: &BattleTeam) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key)?;
    let json = serde_json::to_string(team)?;
    let mut rng = rand::thread_rng();
    let mut nonce_bytes = [0u8; 12];
    for b in nonce_bytes.iter_mut() {
        *b = rng.gen();
    }
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, json.as_bytes())
        .map_err(|e| format!("Encryption failed: {}", e))?;
    let mut output = Vec::with_capacity(12 + ciphertext.len());
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

pub fn decrypt_battle_team(data: &[u8]) -> Option<BattleTeam> {
    if data.len() < 13 || data[0] == b'{' {
        return None;
    }
    let key = derive_encryption_key();
    let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
    let nonce = Nonce::from_slice(&data[..12]);
    let plaintext = cipher.decrypt(nonce, &data[12..]).ok()?;
    let json_str = String::from_utf8(plaintext).ok()?;
    serde_json::from_str(&json_str).ok()
}

/// Canonical data string for signing (excludes signature and prev_hash fields)
pub fn entry_canonical_data(entry: &CaughtPokemon) -> String {
    format!(
        "{}|{}|{}|{}",
        entry.name,
        entry.caught_at.to_rfc3339(),
        entry.ball_used,
        entry.shiny
    )
}

/// Compute the chain hash for an entry (domain-separated)
pub fn compute_entry_hash(entry: &CaughtPokemon, prev_hash: &str) -> String {
    use sha2::Digest;
    let mut hasher = Sha256::new();
    hasher.update(CHAIN_DOMAIN);
    hasher.update(entry_canonical_data(entry).as_bytes());
    hasher.update(prev_hash.as_bytes());
    hex::encode(hasher.finalize())
}

/// HMAC-sign an entry with the derived key
pub fn sign_entry(key: &[u8], entry: &CaughtPokemon, prev_hash: &str) -> String {
    let mut mac =
        <HmacSha256 as HmacMac>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(entry_canonical_data(entry).as_bytes());
    mac.update(prev_hash.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Verify the entire integrity chain. Returns Ok or an error description.
pub fn verify_chain(storage: &PcStorage) -> Result<(), String> {
    let key = derive_signing_key();
    let mut prev_hash = String::from("genesis");

    for (i, entry) in storage.pokemon.iter().enumerate() {
        let entry_prev = entry.prev_hash.as_deref().unwrap_or("genesis");
        if entry_prev != prev_hash {
            return Err(format!(
                "Chain broken at entry {} ({}): prev_hash mismatch",
                i, entry.name
            ));
        }

        let expected_sig = sign_entry(&key, entry, &prev_hash);
        let actual_sig = entry.signature.as_deref().unwrap_or("");
        if actual_sig != expected_sig {
            return Err(format!(
                "Invalid signature at entry {} ({}). Storage may have been tampered with.",
                i, entry.name
            ));
        }

        prev_hash = compute_entry_hash(entry, &prev_hash);
    }

    if let Some(ref stored_hash) = storage.chain_hash {
        if stored_hash != &prev_hash {
            return Err(
                "Final chain hash mismatch. Entries may have been added or removed.".to_string(),
            );
        }
    } else if !storage.pokemon.is_empty() {
        return Err("Missing chain hash on non-empty storage.".to_string());
    }

    Ok(())
}
