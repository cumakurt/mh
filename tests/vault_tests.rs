use aes_gcm::aead::{Aead, OsRng};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use mh::db::Database;
use rand::RngCore;
use sha2::{Digest, Sha256};

#[test]
fn stores_and_reads_encrypted_vault_entries() {
    let temp_dir = tempfile::tempdir().expect("temp dir should be created");
    let database =
        Database::open_path(temp_dir.path().join("history.db")).expect("database should open");

    let passphrase = "test-passphrase";
    let command = "kubectl exec -it pod -- /bin/sh";
    let mut nonce = [0_u8; 12];
    OsRng.fill_bytes(&mut nonce);
    let encrypted = cipher(passphrase)
        .encrypt(Nonce::from_slice(&nonce), command.as_bytes())
        .expect("encryption should succeed");

    let id = database
        .add_vault_entry(&encrypted, &nonce, Some("prod shell"))
        .expect("vault entry should be stored");

    let row = database
        .get_vault_entry(id)
        .expect("vault entry should load");
    let decrypted = cipher(passphrase)
        .decrypt(Nonce::from_slice(&row.nonce), row.encrypted_data.as_slice())
        .expect("decryption should succeed");
    assert_eq!(String::from_utf8(decrypted).expect("utf8"), command);

    let entries = database
        .list_vault_entries()
        .expect("vault list should load");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label.as_deref(), Some("prod shell"));
}

fn cipher(passphrase: &str) -> Aes256Gcm {
    let key = Sha256::digest(passphrase.as_bytes());
    Aes256Gcm::new_from_slice(&key).expect("SHA-256 output is a valid AES-256 key")
}
