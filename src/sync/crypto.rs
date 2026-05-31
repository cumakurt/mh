use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{Context, Result, bail};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn encrypt_payload(token: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher(token)?
        .encrypt(Nonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow::anyhow!("failed to encrypt sync payload"))?;
    let mut payload = Vec::with_capacity(nonce.len() + encrypted.len());
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&encrypted);
    Ok(payload)
}

pub fn decrypt_payload(token: &str, payload: &[u8]) -> Result<Vec<u8>> {
    if payload.len() <= 12 {
        bail!("sync payload is too short");
    }
    let (nonce, encrypted) = payload.split_at(12);
    cipher(token)?
        .decrypt(Nonce::from_slice(nonce), encrypted)
        .map_err(|_| anyhow::anyhow!("failed to decrypt sync payload"))
        .context("sync token may be incorrect")
}

fn cipher(token: &str) -> Result<Aes256Gcm> {
    let key = Sha256::digest(token.as_bytes());
    Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow::anyhow!("invalid AES key length"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_encrypted_payload() {
        let encrypted = encrypt_payload("secret-token", b"payload").expect("encrypt");
        let decrypted = decrypt_payload("secret-token", &encrypted).expect("decrypt");
        assert_eq!(decrypted, b"payload");
    }
}
