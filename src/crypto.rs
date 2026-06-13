
use pqcrypto_dilithium::dilithium2;
use pqcrypto_traits::sign::{DetachedSignature, PublicKey};

pub fn verify_dilithium_signature(
    public_key_bytes: &[u8],
    message: &[u8],
    signature_bytes: &[u8],
) -> bool {
    let expected_pk_size = dilithium2::public_key_bytes();
    if public_key_bytes.len() != expected_pk_size {
        return false;
    }

    let expected_sig_size = dilithium2::signature_bytes();
    if signature_bytes.len() != expected_sig_size {
        return false;
    }

    let public_key = match dilithium2::PublicKey::from_bytes(public_key_bytes) {
        Ok(pk) => pk,
        Err(_) => {
            return false;
        }
    };

    let signature = match dilithium2::DetachedSignature::from_bytes(signature_bytes) {
        Ok(sig) => sig,
        Err(_) => {
            return false;
        }
    };

    match dilithium2::verify_detached_signature(&signature, message, &public_key) {
        Ok(()) => true,
        Err(_) => {
            false
        }
    }
}
