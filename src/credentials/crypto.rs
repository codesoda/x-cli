use super::{unsupported, valid_cookie};
use crate::error::Result;
use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

const IV: [u8; 16] = [b' '; 16];
pub(super) const MAX_ENCRYPTED: usize = 3 + 32 + 4096 + 16;

pub(super) fn derive_key(password: &[u8]) -> Result<Zeroizing<[u8; 16]>> {
    // No empty-key compatibility fallback, base64 decoding, or key generation.
    if password.is_empty() {
        return Err(unsupported());
    }
    let mut key = Zeroizing::new([0u8; 16]);
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1003, &mut *key);
    Ok(key)
}

pub(super) fn decrypt_v24(
    encrypted: &[u8],
    host: &str,
    key: &[u8; 16],
) -> Result<Zeroizing<String>> {
    if !encrypted.starts_with(b"v10")
        || encrypted.len() > MAX_ENCRYPTED
        || encrypted.len() <= 3
        || !(encrypted.len() - 3).is_multiple_of(16)
    {
        return Err(unsupported());
    }
    let mut buffer = Zeroizing::new(encrypted[3..].to_vec());
    // aes/cbc's zeroize features wipe expanded key/IV state on drop. The
    // plaintext buffer is wiped even on padding, digest, UTF-8, or value error.
    let plaintext = cbc::Decryptor::<aes::Aes128>::new(key.into(), (&IV).into())
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|_| unsupported())?;
    let digest = Sha256::digest(host.as_bytes());
    if plaintext.len() <= 32 || plaintext[..32] != digest[..] {
        return Err(unsupported());
    }
    let value = std::str::from_utf8(&plaintext[32..]).map_err(|_| unsupported())?;
    if !valid_cookie(value) {
        return Err(unsupported());
    }
    Ok(Zeroizing::new(value.to_owned()))
}

#[cfg(test)]
pub(super) fn encrypt_fixture(host: &str, value: &[u8], key: &[u8; 16]) -> Vec<u8> {
    use aes::cipher::BlockEncryptMut;
    let mut data = Sha256::digest(host.as_bytes()).to_vec();
    data.extend_from_slice(value);
    let len = data.len();
    data.resize(len + 16, 0);
    let encrypted = cbc::Encryptor::<aes::Aes128>::new(key.into(), (&IV).into())
        .encrypt_padded_mut::<Pkcs7>(&mut data, len)
        .unwrap();
    [b"v10".as_slice(), encrypted].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pbkdf2_known_answer_uses_text_bytes() {
        // Independently generated using Python hashlib.pbkdf2_hmac.
        assert_eq!(
            &*derive_key(b"synthetic-password").unwrap(),
            &[
                0xad, 0x47, 0xd0, 0x40, 0xc1, 0x84, 0xe4, 0x03, 0xf3, 0xaf, 0xc7, 0xb0, 0x40, 0x54,
                0xec, 0x52,
            ]
        );
        assert!(derive_key(b"").is_err());
    }

    #[test]
    fn independently_generated_openssl_ciphertext() {
        // Python hashlib KDF/digest + OpenSSL enc -aes-128-cbc, synthetic only.
        let hex = "7631302e7f3d285c73ae062629059333c36f44ae8a7e04f854eda3f997fbc615a600a4e8d3487de103cd7928324e835b19d75847fb720e259be3af8e4766c2872bf25a";
        let encrypted: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let key = derive_key(b"synthetic-password").unwrap();
        assert_eq!(
            &*decrypt_v24(&encrypted, ".x.com", &key).unwrap(),
            "synthetic-cookie"
        );
    }

    #[test]
    fn exact_stored_host_digest_is_required() {
        let key = derive_key(b"synthetic-password").unwrap();
        for host in [".x.com", "x.com"] {
            let encrypted = encrypt_fixture(host, b"synthetic-cookie", &key);
            assert_eq!(
                &*decrypt_v24(&encrypted, host, &key).unwrap(),
                "synthetic-cookie"
            );
            let other = if host.starts_with('.') {
                "x.com"
            } else {
                ".x.com"
            };
            assert!(decrypt_v24(&encrypted, other, &key).is_err());
        }
    }

    #[test]
    fn malformed_protected_and_invalid_plaintext_fail_closed() {
        let key = derive_key(b"synthetic-password").unwrap();
        for value in [b"".as_slice(), b"a\r\nb", b"bad;cookie", &[0xff]] {
            let encrypted = encrypt_fixture(".x.com", value, &key);
            assert!(decrypt_v24(&encrypted, ".x.com", &key).is_err());
        }
        for encrypted in [
            b"".as_slice(),
            b"v10",
            b"v10abc",
            b"v20protected",
            b"APPBprotected",
            b"plaintext",
        ] {
            assert!(decrypt_v24(encrypted, ".x.com", &key).is_err());
        }
        let mut encrypted = encrypt_fixture(".x.com", b"synthetic-cookie", &key);
        // Value length 16 gives a full block of 0x10 padding. Flipping the
        // preceding block's last byte makes the last padding byte zero.
        let i = encrypted.len() - 17;
        encrypted[i] ^= 0x10;
        assert!(decrypt_v24(&encrypted, ".x.com", &key).is_err());
        let encrypted = encrypt_fixture(".x.com", b"synthetic-cookie", &key);
        assert!(decrypt_v24(&encrypted, ".x.com", &[0; 16]).is_err());
    }
}
