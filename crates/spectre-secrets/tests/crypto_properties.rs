//! Property-based tests for spectre-secrets crypto module
//!
//! Uses proptest to verify cryptographic invariants hold across
//! randomly generated inputs (passwords, salts, plaintext).

use proptest::prelude::*;
use spectre_secrets::{generate_salt, CryptoEngine};

// --- Strategies ---

/// Arbitrary non-empty password (1..256 bytes)
fn password_strategy() -> impl Strategy<Value = String> {
    "[\\x01-\\x7f]{1,256}"
}

/// Arbitrary salt (8..64 bytes, Argon2 minimum is 8)
fn salt_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 8..64)
}

/// Arbitrary plaintext (0..4096 bytes)
fn plaintext_strategy() -> impl Strategy<Value = Vec<u8>> {
    prop::collection::vec(any::<u8>(), 0..4096)
}

// --- Property Tests ---

proptest! {
    /// Encrypt-then-decrypt always recovers the original plaintext.
    #[test]
    fn prop_encrypt_decrypt_roundtrip(
        password in password_strategy(),
        plaintext in plaintext_strategy(),
    ) {
        let salt = generate_salt();
        let engine = CryptoEngine::new(&password, &salt).unwrap();

        let ciphertext = engine.encrypt(&plaintext).unwrap();
        let decrypted = engine.decrypt(&ciphertext).unwrap();

        prop_assert_eq!(decrypted, plaintext);
    }

    /// KDF is deterministic: same (password, salt) always produces a key
    /// that can decrypt data encrypted with the same (password, salt).
    #[test]
    fn prop_kdf_determinism(
        password in password_strategy(),
        salt in salt_strategy(),
        plaintext in plaintext_strategy(),
    ) {
        let engine1 = CryptoEngine::new(&password, &salt).unwrap();
        let engine2 = CryptoEngine::new(&password, &salt).unwrap();

        let ciphertext = engine1.encrypt(&plaintext).unwrap();
        let decrypted = engine2.decrypt(&ciphertext).unwrap();

        prop_assert_eq!(decrypted, plaintext);
    }

    /// Different passwords produce keys that cannot decrypt each other's ciphertext.
    #[test]
    fn prop_wrong_password_fails(
        password1 in password_strategy(),
        password2 in password_strategy(),
        plaintext in prop::collection::vec(any::<u8>(), 1..512),
    ) {
        prop_assume!(password1 != password2);

        let salt = generate_salt();
        let engine1 = CryptoEngine::new(&password1, &salt).unwrap();
        let engine2 = CryptoEngine::new(&password2, &salt).unwrap();

        let ciphertext = engine1.encrypt(&plaintext).unwrap();
        let result = engine2.decrypt(&ciphertext);

        prop_assert!(result.is_err());
    }

    /// Different salts with the same password produce keys that cannot
    /// decrypt each other's ciphertext.
    #[test]
    fn prop_different_salts_fail(
        password in password_strategy(),
        salt1 in salt_strategy(),
        salt2 in salt_strategy(),
        plaintext in prop::collection::vec(any::<u8>(), 1..512),
    ) {
        prop_assume!(salt1 != salt2);

        let engine1 = CryptoEngine::new(&password, &salt1).unwrap();
        let engine2 = CryptoEngine::new(&password, &salt2).unwrap();

        let ciphertext = engine1.encrypt(&plaintext).unwrap();
        let result = engine2.decrypt(&ciphertext);

        prop_assert!(result.is_err());
    }

    /// Ciphertext is always longer than plaintext (nonce + auth tag overhead).
    /// AES-256-GCM: 12 bytes nonce + 16 bytes tag = 28 bytes overhead.
    #[test]
    fn prop_ciphertext_has_overhead(
        password in password_strategy(),
        plaintext in plaintext_strategy(),
    ) {
        let salt = generate_salt();
        let engine = CryptoEngine::new(&password, &salt).unwrap();

        let ciphertext = engine.encrypt(&plaintext).unwrap();

        // nonce (12) + plaintext + auth tag (16)
        prop_assert_eq!(ciphertext.len(), plaintext.len() + 28);
    }

    /// Each encryption of the same plaintext produces different ciphertext
    /// (due to random nonce).
    #[test]
    fn prop_encryption_is_non_deterministic(
        password in password_strategy(),
        plaintext in prop::collection::vec(any::<u8>(), 1..512),
    ) {
        let salt = generate_salt();
        let engine = CryptoEngine::new(&password, &salt).unwrap();

        let ct1 = engine.encrypt(&plaintext).unwrap();
        let ct2 = engine.encrypt(&plaintext).unwrap();

        // Both decrypt to the same plaintext
        let d1 = engine.decrypt(&ct1).unwrap();
        let d2 = engine.decrypt(&ct2).unwrap();
        prop_assert_eq!(d1, plaintext.clone());
        prop_assert_eq!(d2, plaintext);

        // Ciphertexts should differ (random nonce)
        prop_assert_ne!(ct1, ct2);
    }

    /// generate_salt() produces unique salts (no collisions in sequence).
    #[test]
    fn prop_salts_are_unique(_ in 0..100u32) {
        let salt1 = generate_salt();
        let salt2 = generate_salt();
        prop_assert_ne!(salt1, salt2);
    }

    /// Salt shorter than 8 bytes is always rejected.
    #[test]
    fn prop_short_salt_rejected(
        password in password_strategy(),
        salt in prop::collection::vec(any::<u8>(), 0..8),
    ) {
        let result = CryptoEngine::new(&password, &salt);
        prop_assert!(result.is_err());
    }

    /// Truncated ciphertext always fails to decrypt.
    #[test]
    fn prop_truncated_ciphertext_fails(
        password in password_strategy(),
        plaintext in prop::collection::vec(any::<u8>(), 1..512),
        truncate_at in 0..28usize,
    ) {
        let salt = generate_salt();
        let engine = CryptoEngine::new(&password, &salt).unwrap();
        let ciphertext = engine.encrypt(&plaintext).unwrap();

        // Truncate to less than nonce+tag overhead
        let truncated = &ciphertext[..truncate_at.min(ciphertext.len())];
        let result = engine.decrypt(truncated);
        prop_assert!(result.is_err());
    }

    /// Bit-flipping any byte in ciphertext causes decryption to fail
    /// (authenticated encryption integrity).
    #[test]
    fn prop_tampered_ciphertext_fails(
        password in password_strategy(),
        plaintext in prop::collection::vec(any::<u8>(), 1..256),
        flip_pos in any::<prop::sample::Index>(),
    ) {
        let salt = generate_salt();
        let engine = CryptoEngine::new(&password, &salt).unwrap();
        let mut ciphertext = engine.encrypt(&plaintext).unwrap();

        // Flip a bit at a random position
        let idx = flip_pos.index(ciphertext.len());
        ciphertext[idx] ^= 0x01;

        let result = engine.decrypt(&ciphertext);
        prop_assert!(result.is_err());
    }
}
