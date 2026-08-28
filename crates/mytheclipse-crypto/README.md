# mytheclipse-crypto

Safe, one-line security helpers that are easy to get wrong when hand-rolled:

- **Argon2id** password hashing & verification (PHC-encoded, RFC 9106-style).
- **AES-256-GCM** authenticated encryption with a fresh random nonce per op.
- **JWT** (HS256) token generation & validation.
- **Key rotation** via `KeyRing` — reads keep working with the previous key
  during a rotation window.

## Features

- `password` (default) — Argon2id hashing.
- `encryption` (default) — AES-256-GCM.
- `tokens` (default) — JSON Web Tokens.

Zero features enabled by default? No — all three are on, but each is cheap and
independent.

## Usage

```rust
use mytheclipse_crypto::{PasswordHasher, Encryptor, TokenSigner};

let hasher = PasswordHasher::new();
let hash = hasher.hash("letmein").unwrap();
assert!(hasher.verify(&hash, "letmein"));

let enc = Encryptor::new(&[0u8; 32]);
let (nonce, ct) = enc.encrypt(b"secret");
assert_eq!(enc.decrypt(&nonce, &ct).unwrap(), b"secret");

let signer = TokenSigner::new("my-secret");
let token = signer.sign(&serde_json::json!({"sub":"u1"}), std::time::Duration::from_secs(3600)).unwrap();
assert_eq!(signer.verify(&token).unwrap()["sub"], "u1");
```
