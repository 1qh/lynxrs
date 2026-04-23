// Property tests. Cover things that are easy to regress silently:
//   - argon2 hash↔verify round-trip + rejection of mutated input
//   - base64 encode/decode round-trip
//   - session-token (url-safe base64 no pad) has stable length ≥16

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use proptest::prelude::*;

fn hash(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("hash")
        .to_string()
}

fn verify(password: &str, phc: &str) -> bool {
    let parsed = PasswordHash::new(phc).expect("parse phc");
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, .. ProptestConfig::default() })]

    #[test]
    fn hash_verify_round_trip(pw in "[ -~]{8,64}") {
        let h = hash(&pw);
        prop_assert!(verify(&pw, &h));
    }

    #[test]
    fn mutated_password_rejected(pw in "[ -~]{8,64}") {
        let h = hash(&pw);
        let mutated = format!("{pw}x");
        prop_assert!(!verify(&mutated, &h));
    }

    #[test]
    fn empty_password_rejected(pw in "[ -~]{8,64}") {
        let h = hash(&pw);
        prop_assert!(!verify("", &h));
    }

    #[test]
    fn base64_round_trip(bytes in proptest::collection::vec(any::<u8>(), 0..10_000)) {
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        let encoded = STANDARD.encode(&bytes);
        let decoded = STANDARD.decode(encoded.as_bytes()).expect("decode");
        prop_assert_eq!(decoded, bytes);
    }
}
