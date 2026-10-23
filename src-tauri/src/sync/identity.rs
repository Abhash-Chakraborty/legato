//! Who this device is to its phone: a device ID and a self-signed certificate,
//! known by its fingerprint. Both TLS ends pin fingerprints instead of
//! checking a chain.

use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{
    ClientConfig, DigitallySignedStruct, DistinguishedName, ServerConfig, SignatureScheme,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The name in every Legato certificate. Nothing checks it.
const CERT_NAME: &str = "legato";

#[derive(Clone, Serialize, Deserialize)]
pub struct Identity {
    pub device_id: String,
    /// Certificate DER, base64.
    cert: String,
    /// PKCS#8 private key DER, base64.
    key: String,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identity")
            .field("device_id", &self.device_id)
            .field("fingerprint", &self.fingerprint())
            .finish_non_exhaustive()
    }
}

impl Identity {
    pub fn generate() -> Result<Self> {
        let key = rcgen::KeyPair::generate()?;
        let cert = rcgen::CertificateParams::new(vec![CERT_NAME.to_string()])?.self_signed(&key)?;
        Ok(Self {
            device_id: uuid(),
            cert: STANDARD.encode(cert.der()),
            key: STANDARD.encode(key.serialize_der()),
        })
    }

    #[allow(dead_code)] // The stand-in phone keeps its identity in a file.
    /// Reads the identity kept at `path`, making and keeping one the first
    /// time.
    pub fn load_or_create(path: &Path) -> Result<Self> {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(identity) = serde_json::from_str::<Identity>(&text) {
                return Ok(identity);
            }
            log::warn!("The sync identity at {path:?} is unreadable; making a new one");
        }
        let identity = Self::generate()?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, serde_json::to_string(&identity)?)
            .with_context(|| format!("writing {path:?}"))?;
        Ok(identity)
    }

    pub fn cert_der(&self) -> CertificateDer<'static> {
        CertificateDer::from(STANDARD.decode(&self.cert).unwrap_or_default())
    }

    fn key_der(&self) -> PrivateKeyDer<'static> {
        PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
            STANDARD.decode(&self.key).unwrap_or_default(),
        ))
    }

    pub fn fingerprint(&self) -> String {
        fingerprint(&self.cert_der())
    }

    /// The PC's side: any client certificate gets through the handshake, and
    /// the session then checks its fingerprint.
    pub fn server_config(&self) -> Result<Arc<ServerConfig>> {
        let provider = provider();
        let config = ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(Arc::new(AnyClientCert(provider)))
            .with_single_cert(vec![self.cert_der()], self.key_der())?;
        Ok(Arc::new(config))
    }

    /// The phone's side: the PC must have `fingerprint`.
    #[allow(dead_code)] // Used by phone/core, not the app.
    pub fn client_config(&self, fingerprint: &str) -> Result<Arc<ClientConfig>> {
        let provider = provider();
        let config = ClientConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(Pinned {
                provider,
                fingerprint: fingerprint.to_string(),
                encode: self::fingerprint,
                whose: "the PC's",
            }))
            .with_client_auth_cert(vec![self.cert_der()], self.key_der())?;
        Ok(Arc::new(config))
    }
}

#[allow(dead_code)] // The phone's side, used by phone/core.
pub fn server_name() -> ServerName<'static> {
    ServerName::try_from(CERT_NAME).expect("a valid DNS name")
}

pub fn fingerprint(cert: &CertificateDer<'_>) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(cert.as_ref()))
}

/// A certificate's SHA-256 as 64 lowercase hex digits, the way Inyon's
/// hub gives it.
pub fn sha256_hex(cert: &CertificateDer<'_>) -> String {
    Sha256::digest(cert.as_ref())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A TLS client that trusts exactly one server certificate, the one whose
/// SHA-256 is `sha256_hex`, whatever name it carries: for an Inyon hub
/// that serves its own certificate. Speaks HTTP/1.1, which both the hub's
/// API and its WebSocket use.
#[allow(dead_code)] // The app's side; phone/core doesn't talk to Inyon.
pub fn hub_client_config(sha256_hex: &str) -> Result<ClientConfig> {
    let provider = provider();
    let mut config = ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Pinned {
            provider,
            fingerprint: sha256_hex.to_ascii_lowercase(),
            encode: self::sha256_hex,
            whose: "the Inyon hub's",
        }))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| anyhow!("no randomness"))
        .expect("the system random source works");
    bytes
}

/// A random version 4 UUID.
pub fn uuid() -> String {
    let mut b = random_bytes::<16>();
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// A pairing secret: 16 random bytes, base64url.
pub fn secret() -> String {
    URL_SAFE_NO_PAD.encode(random_bytes::<16>())
}

/// Compares secrets without stopping at the first difference.
pub fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

#[derive(Debug)]
struct AnyClientCert(Arc<CryptoProvider>);

impl ClientCertVerifier for AnyClientCert {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn client_auth_mandatory(&self) -> bool {
        true
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Trusts the one certificate whose fingerprint, as `encode` writes it, is
/// `fingerprint`.
#[derive(Debug)]
struct Pinned {
    provider: Arc<CryptoProvider>,
    fingerprint: String,
    encode: fn(&CertificateDer<'_>) -> String,
    /// Whose certificate, for the error.
    whose: &'static str,
}

impl ServerCertVerifier for Pinned {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if same_secret(&(self.encode)(end_entity), &self.fingerprint) {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General(format!(
                "{} certificate doesn't match the paired one",
                self.whose
            )))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_keeps_its_fingerprint_across_a_reload() {
        let dir = std::env::temp_dir().join(format!("legato-identity-{}", uuid()));
        let path = dir.join("identity.json");
        let first = Identity::load_or_create(&path).unwrap();
        let again = Identity::load_or_create(&path).unwrap();
        assert_eq!(first.fingerprint(), again.fingerprint());
        assert_eq!(first.device_id, again.device_id);
        assert_eq!(first.fingerprint().len(), 43);
        assert!(!format!("{first:?}").contains(&first.key));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn uuids_and_secrets_look_right() {
        let id = uuid();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert_ne!(uuid(), id);
        assert_eq!(secret().len(), 22);
        assert!(same_secret("abc", "abc"));
        assert!(!same_secret("abc", "abd"));
        assert!(!same_secret("abc", "abcd"));
    }
}
