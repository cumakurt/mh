use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::{Context, Result, bail};
use comfy_table::{Cell, Table, presets::UTF8_FULL};
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::cli::{VaultArgs, VaultCommand};
use crate::command_exec::execute_shell_command;
use crate::config::AppConfig;
use crate::db::Database;
use crate::execution_policy::ensure_execution_allowed;

const KEYRING_SERVICE: &str = "mh-vault";
const KEYRING_USER: &str = "passphrase";

pub fn run(args: VaultArgs) -> Result<()> {
    match args.command {
        VaultCommand::Add { command, label } => add(command, label),
        VaultCommand::List => list(),
        VaultCommand::Run { id, dry_run } => run_entry(id, dry_run),
        VaultCommand::Delete { id } => delete(id),
        VaultCommand::Unlock => {
            let config = AppConfig::load()?;
            let _ = passphrase(&config)?;
            println!("Vault passphrase accepted for this operation");
            Ok(())
        }
        VaultCommand::Lock => {
            println!("Vault has no persistent unlocked state");
            Ok(())
        }
    }
}

fn add(command: String, label: Option<String>) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let passphrase = passphrase(&config)?;
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher(&passphrase)?
        .encrypt(Nonce::from_slice(&nonce), command.as_bytes())
        .map_err(|_| anyhow::anyhow!("failed to encrypt vault command"))?;
    let id = database.add_vault_entry(&encrypted, &nonce, label.as_deref())?;
    println!("Added vault entry {id}");
    Ok(())
}

fn list() -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let rows = database.list_vault_entries()?;

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["ID", "Created", "Label"]);
    for row in rows {
        table.add_row(vec![
            Cell::new(row.id),
            Cell::new(row.created_at),
            Cell::new(row.label.unwrap_or_else(|| "-".to_string())),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn run_entry(id: i64, dry_run: bool) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let row = database.get_vault_entry(id)?;
    let passphrase = passphrase(&config)?;
    let command = decrypt_command(&passphrase, &row.encrypted_data, &row.nonce)?;

    if dry_run {
        println!("{command}");
        return Ok(());
    }

    let hostname = hostname::get()
        .ok()
        .map(|value| value.to_string_lossy().to_string());
    ensure_execution_allowed(&config, &command, hostname.as_deref(), None)?;

    let status = execute_shell_command(&command, None::<&Path>)?;
    if !status.success() {
        bail!("vault command exited with status {status}");
    }
    Ok(())
}

fn delete(id: i64) -> Result<()> {
    let config = AppConfig::load()?;
    let database = Database::open(&config)?;
    let deleted = database.delete_vault_entry(id)?;
    println!("Deleted {deleted} vault entry");
    Ok(())
}

fn passphrase(config: &AppConfig) -> Result<String> {
    static WARNED_ENV_PASSPHRASE: AtomicBool = AtomicBool::new(false);

    if let Ok(value) = std::env::var("MH_VAULT_PASSPHRASE")
        && !value.is_empty()
    {
        if !WARNED_ENV_PASSPHRASE.swap(true, Ordering::Relaxed) {
            eprintln!(
                "warning: MH_VAULT_PASSPHRASE is visible to other processes; prefer keyring or an interactive prompt"
            );
        }
        return Ok(value);
    }

    if config.vault.use_keyring
        && let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
        && let Ok(value) = entry.get_password()
        && !value.is_empty()
    {
        return Ok(value);
    }

    let value = rpassword::prompt_password("Vault passphrase: ")
        .context("failed to read vault passphrase")?;

    if config.vault.use_keyring
        && let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
    {
        let _ = entry.set_password(&value);
    }

    Ok(value)
}

fn cipher(passphrase: &str) -> Result<Aes256Gcm> {
    let key = Sha256::digest(passphrase.as_bytes());
    Aes256Gcm::new_from_slice(&key).map_err(|_| anyhow::anyhow!("invalid AES key length"))
}

fn decrypt_command(passphrase: &str, encrypted_data: &[u8], nonce: &[u8]) -> Result<String> {
    let plaintext = cipher(passphrase)?
        .decrypt(Nonce::from_slice(nonce), encrypted_data)
        .map_err(|_| anyhow::anyhow!("failed to decrypt vault command"))?;
    Ok(String::from_utf8(plaintext)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypts_encrypted_command() {
        let mut nonce = [0_u8; 12];
        nonce[0] = 1;
        let encrypted = cipher("secret")
            .expect("cipher")
            .encrypt(Nonce::from_slice(&nonce), b"echo ok".as_slice())
            .expect("encryption should succeed");
        let command =
            decrypt_command("secret", &encrypted, &nonce).expect("decryption should succeed");
        assert_eq!(command, "echo ok");
    }
}
