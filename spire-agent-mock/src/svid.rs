use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType,
    ExtendedKeyUsagePurpose, IsCa, KeyPair, KeyUsagePurpose, SanType,
};
use time::{Duration, OffsetDateTime};

/// Represents a SPIFFE X.509 SVID with its private key and CA bundle
pub struct X509Svid {
    /// The SPIFFE ID (e.g., spiffe://example.org/workload)
    pub spiffe_id: String,
    /// DER-encoded certificate chain (leaf certificate first)
    pub cert_chain_der: Vec<u8>,
    /// DER-encoded PKCS#8 private key
    pub private_key_der: Vec<u8>,
    /// DER-encoded CA certificate (trust bundle)
    pub bundle_der: Vec<u8>,
}

/// Configuration for SVID generation
pub struct SvidConfig {
    pub trust_domain: String,
    pub workload_path: String,
    pub ttl_seconds: i64,
}

impl Default for SvidConfig {
    fn default() -> Self {
        Self {
            trust_domain: "example.org".to_string(),
            workload_path: "/workload".to_string(),
            ttl_seconds: 30,
        }
    }
}

/// Generator for SPIFFE X.509 SVIDs
pub struct SvidGenerator {
    config: SvidConfig,
    ca_cert: Certificate,
    ca_key_pair: KeyPair,
    ca_cert_der: Vec<u8>,
}

impl SvidGenerator {
    /// Create a new SVID generator with the given configuration
    pub fn new(config: SvidConfig) -> Self {
        let (ca_cert, ca_key_pair, ca_cert_der) = Self::generate_ca(&config.trust_domain);
        Self {
            config,
            ca_cert,
            ca_key_pair,
            ca_cert_der,
        }
    }

    /// Generate a CA certificate for the trust domain
    fn generate_ca(trust_domain: &str) -> (Certificate, KeyPair, Vec<u8>) {
        let mut params = CertificateParams::default();

        // Set distinguished name
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, format!("{} CA", trust_domain));
        dn.push(DnType::OrganizationName, trust_domain);
        params.distinguished_name = dn;

        // CA certificate settings
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];

        // Set validity period (1 year for CA)
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + Duration::days(365);

        // Add SPIFFE trust domain as URI SAN
        let trust_domain_uri = format!("spiffe://{}", trust_domain);
        params.subject_alt_names = vec![SanType::URI(trust_domain_uri.parse().unwrap())];

        // Generate key pair
        let key_pair = KeyPair::generate().unwrap();

        // Generate self-signed CA certificate
        let ca_cert = params.self_signed(&key_pair).unwrap();
        let ca_cert_der = ca_cert.der().to_vec();

        (ca_cert, key_pair, ca_cert_der)
    }

    /// Generate a new X.509 SVID
    pub fn generate_svid(&self) -> X509Svid {
        let spiffe_id = format!(
            "spiffe://{}{}",
            self.config.trust_domain, self.config.workload_path
        );

        // Create workload certificate parameters
        let mut params = CertificateParams::default();

        // Set distinguished name
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, &spiffe_id);
        dn.push(DnType::OrganizationName, &self.config.trust_domain);
        params.distinguished_name = dn;

        // Leaf certificate (not a CA)
        params.is_ca = IsCa::NoCa;
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyEncipherment,
        ];
        params.extended_key_usages = vec![
            ExtendedKeyUsagePurpose::ServerAuth,
            ExtendedKeyUsagePurpose::ClientAuth,
        ];

        // Set validity period based on TTL
        let now = OffsetDateTime::now_utc();
        params.not_before = now;
        params.not_after = now + Duration::seconds(self.config.ttl_seconds);

        // SPIFFE ID as URI SAN - this is required by SPIFFE spec
        params.subject_alt_names = vec![SanType::URI(spiffe_id.parse().unwrap())];

        // Generate key pair for the workload
        let key_pair = KeyPair::generate().unwrap();

        // Sign with CA
        let cert = params
            .signed_by(&key_pair, &self.ca_cert, &self.ca_key_pair)
            .unwrap();

        X509Svid {
            spiffe_id,
            cert_chain_der: cert.der().to_vec(),
            private_key_der: key_pair.serialize_der(),
            bundle_der: self.ca_cert_der.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_svid() {
        let config = SvidConfig::default();
        let generator = SvidGenerator::new(config);
        let svid = generator.generate_svid();

        assert_eq!(svid.spiffe_id, "spiffe://example.org/workload");
        assert!(!svid.cert_chain_der.is_empty());
        assert!(!svid.private_key_der.is_empty());
        assert!(!svid.bundle_der.is_empty());
    }

    #[test]
    fn test_custom_config() {
        let config = SvidConfig {
            trust_domain: "test.domain".to_string(),
            workload_path: "/my/service".to_string(),
            ttl_seconds: 60,
        };
        let generator = SvidGenerator::new(config);
        let svid = generator.generate_svid();

        assert_eq!(svid.spiffe_id, "spiffe://test.domain/my/service");
    }
}
