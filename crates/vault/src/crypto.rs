//! Primitif crypto murni — tidak ada I/O database di sini, supaya gampang
//! di-unit-test dan di-audit terpisah dari logic storage.

use crate::VaultError;
use argon2::Argon2;
use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand::RngCore;

const NONCE_LEN: usize = 12;

/// Turunkan encryption key 256-bit dari master password + salt pakai
/// Argon2id. Parameter default crate `argon2` (memory-hard, m=19MiB,
/// t=2, p=1) sengaja "berat" untuk memperlambat brute-force offline
/// kalau file vault dicuri.
pub fn derive_key(master_password: &str, salt: &[u8]) -> Result<[u8; 32], VaultError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(master_password.as_bytes(), salt, &mut key)
        .map_err(|e| VaultError::Crypto(format!("gagal derive key: {e}")))?;
    Ok(key)
}

/// Generate salt random 16 byte, dipakai sekali waktu vault pertama
/// kali dibuat (`VaultStore::initialize`) dan disimpan permanen di
/// `vault_meta` supaya `derive_key` selalu hasilnya sama untuk master
/// password yang sama.
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    salt
}

/// Enkripsi satu blob rahasia (mis. private key PEM atau password host)
/// pakai ChaCha20-Poly1305. Nonce di-generate random per-panggilan dan
/// disimpan bareng ciphertext (nonce tidak rahasia, cuma harus unik per
/// key) — layout output: `nonce (12 byte) || ciphertext+tag`.
pub fn encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, VaultError> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let mut nonce_bytes = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| VaultError::Crypto(format!("gagal encrypt: {e}")))?;

    let mut out = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Kebalikan `encrypt`: pisahkan 12 byte nonce pertama, decrypt sisanya.
/// Kalau auth tag tidak cocok (data korup ATAU key salah, mis. master
/// password salah), `Aead::decrypt` mengembalikan error tanpa membuka
/// isi apapun — itu dipetakan ke `VaultError::Crypto` di sini.
pub fn decrypt(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>, VaultError> {
    if ciphertext.len() < NONCE_LEN {
        return Err(VaultError::Crypto("ciphertext terlalu pendek".into()));
    }
    let (nonce_bytes, body) = ciphertext.split_at(NONCE_LEN);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce = Nonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, body)
        .map_err(|e| VaultError::Crypto(format!("gagal decrypt (auth tag tidak cocok): {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let salt = generate_salt();
        let key = derive_key("master-password-yang-kuat", &salt).unwrap();
        let plaintext = b"password-ssh-rahasia";

        let ciphertext = encrypt(&key, plaintext).unwrap();
        assert_ne!(ciphertext.as_slice(), plaintext.as_slice());

        let decrypted = decrypt(&key, &ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn decrypt_dengan_key_salah_ditolak() {
        let salt = generate_salt();
        let key_benar = derive_key("password-benar", &salt).unwrap();
        let key_salah = derive_key("password-salah", &salt).unwrap();

        let ciphertext = encrypt(&key_benar, b"rahasia").unwrap();
        assert!(decrypt(&key_salah, &ciphertext).is_err());
    }

    #[test]
    fn derive_key_deterministik_untuk_salt_sama() {
        let salt = generate_salt();
        let key1 = derive_key("password", &salt).unwrap();
        let key2 = derive_key("password", &salt).unwrap();
        assert_eq!(key1, key2);
    }
}
