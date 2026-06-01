use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use chrono::{SecondsFormat, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::config::PolicyConfig;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyPackPayload {
    pub name: String,
    pub version: u32,
    pub signed_at: String,
    pub policy: PolicyConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedPolicyPack {
    pub payload: PolicyPackPayload,
    pub signature: String,
}

pub fn sign_policy(policy: PolicyConfig, key: &str) -> Result<SignedPolicyPack> {
    ensure_key(key)?;
    let payload = PolicyPackPayload {
        name: "mh-policy-pack".to_string(),
        version: 1,
        signed_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        policy,
    };
    let signature = signature_for(&payload, key)?;
    Ok(SignedPolicyPack { payload, signature })
}

pub fn read_pack(path: &Path) -> Result<SignedPolicyPack> {
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn verify_pack(pack: &SignedPolicyPack, key: &str) -> Result<()> {
    ensure_key(key)?;
    let expected = signature_for(&pack.payload, key)?;
    if !constant_time_eq(pack.signature.as_bytes(), expected.as_bytes()) {
        bail!("policy pack signature verification failed");
    }
    Ok(())
}

pub fn write_pack(path: &Path, pack: &SignedPolicyPack) -> Result<()> {
    let content = serde_json::to_vec_pretty(pack).context("failed to serialize policy pack")?;
    crate::config::write_private_file(path, &content)
        .with_context(|| format!("failed to write policy pack {}", path.display()))
}

fn signature_for(payload: &PolicyPackPayload, key: &str) -> Result<String> {
    let canonical = serde_json::to_vec(payload).context("failed to serialize policy payload")?;
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .context("failed to initialize policy pack signer")?;
    mac.update(b"mh-policy-pack-v1\0");
    mac.update(&canonical);
    Ok(format!(
        "hmac-sha256:{}",
        encode_hex(&mac.finalize().into_bytes())
    ))
}

fn ensure_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        bail!("policy pack signing key must not be empty");
    }
    Ok(())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0_u8, |acc, (left, right)| acc | (left ^ right))
        == 0
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_pack_verifies_with_same_key() {
        let pack = sign_policy(crate::policy::default_policy_config(), "secret").expect("sign");
        verify_pack(&pack, "secret").expect("verify");
        assert!(verify_pack(&pack, "wrong").is_err());
    }
}
