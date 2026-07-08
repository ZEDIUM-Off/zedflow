//! PKCE helpers ported from Pi's `packages/ai/src/utils/oauth/pkce.ts`.

use std::error::Error as StdError;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

const VERIFIER_BYTES: usize = 32;

/// Generated PKCE verifier and SHA-256 code challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkce {
    /// Base64url-encoded random verifier.
    pub verifier: String,
    /// Base64url-encoded SHA-256 digest of [`Self::verifier`].
    pub challenge: String,
}

/// Errors returned while generating PKCE values.
#[derive(Debug)]
pub struct PkceError {
    source: getrandom::Error,
}

impl fmt::Display for PkceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to generate PKCE verifier bytes: {}",
            self.source
        )
    }
}

impl StdError for PkceError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.source)
    }
}

impl From<getrandom::Error> for PkceError {
    fn from(source: getrandom::Error) -> Self {
        Self { source }
    }
}

/// Generates a PKCE code verifier and S256 challenge.
///
/// The verifier is 32 random bytes encoded with unpadded base64url, matching
/// Pi's Web Crypto implementation.
///
/// # Errors
///
/// Returns [`PkceError`] when the operating system random source cannot fill
/// the verifier bytes.
pub async fn generate_pkce() -> Result<Pkce, PkceError> {
    let mut verifier_bytes = [0_u8; VERIFIER_BYTES];
    getrandom::fill(&mut verifier_bytes)?;

    let verifier = base64url_encode(&verifier_bytes);
    let challenge = base64url_encode(&Sha256::digest(verifier.as_bytes()));

    Ok(Pkce {
        verifier,
        challenge,
    })
}

fn base64url_encode(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use futures::executor::block_on;
    use sha2::{Digest, Sha256};

    use super::{base64url_encode, generate_pkce};

    #[test]
    fn base64url_matches_pi_encoding() {
        assert_eq!(base64url_encode(&[251, 255, 255]), "-___");
        assert_eq!(base64url_encode(b"hello"), "aGVsbG8");
        assert!(!base64url_encode(b"hello").contains('='));
    }

    #[test]
    fn generate_pkce_returns_verifier_and_sha256_challenge() {
        let pkce = block_on(generate_pkce()).expect("random bytes are available");
        let expected_challenge = base64url_encode(&Sha256::digest(pkce.verifier.as_bytes()));

        assert_eq!(pkce.verifier.len(), 43);
        assert_eq!(pkce.challenge, expected_challenge);
        assert!(!pkce.verifier.contains('='));
        assert!(!pkce.challenge.contains('='));
    }
}
