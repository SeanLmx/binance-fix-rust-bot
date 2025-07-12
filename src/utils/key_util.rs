use std::env;
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{SigningKey, SECRET_KEY_LENGTH};


pub fn load_signing_key() -> anyhow::Result<SigningKey> {
    let key_b64 = env::var("BINANCE_PRIVATE_KEY_BASE64")?;
    let key_bytes = general_purpose::STANDARD.decode(key_b64)?;
    
    let private_key_bytes = if key_bytes.len() == 48 && key_bytes[0] == 0x30 {
        &key_bytes[16..48]
    } else if key_bytes.len() == SECRET_KEY_LENGTH {
        &key_bytes[..]
    } else {
        anyhow::bail!("Invalid Ed25519 private key format");
    };

    Ok(SigningKey::from_bytes(private_key_bytes.try_into()?))
}
