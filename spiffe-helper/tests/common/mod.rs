use spire_agent_mock::server::{MockWorkloadApi, SpiffeWorkloadApiServer};
use spire_agent_mock::svid::SvidConfig;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

const VALIDITY_LEEWAY_SECONDS: i64 = 15;

/// Wait for the socket file to exist (with timeout).
pub async fn assert_socket_ready(socket_path: &Path) {
    // max delay 2 seconds
    let max_attempts = 10;
    let delay = std::time::Duration::from_millis(200);
    let mut ready = false;

    for _ in 0..max_attempts {
        if socket_path.exists() {
            ready = true;
            break;
        }
        tokio::time::sleep(delay).await;
    }

    assert!(
        ready,
        "Socket file was not created within timeout: {}",
        socket_path.display()
    );
}

pub fn assert_x509_cert(path: &Path) -> Vec<u8> {
    assert!(
        path.exists(),
        "Certificate file does not exist: {}",
        path.display()
    );

    let content = fs::read_to_string(path).expect("Failed to read certificate");
    let cert_pem = pem::parse(&content).expect("Failed to read parse certificate PEM");
    assert_eq!(cert_pem.tag, "CERTIFICATE", "Unexpected cert PEM tag");

    let (_bytes, cert) = x509_parser::parse_x509_certificate(&cert_pem.contents)
        .expect("Failed to parse cert in x.509");

    let expected_spiffe_id = "spiffe://example.org/test/workload";
    let expected_org = "example.org";
    let subject = cert.subject();
    let has_cn = subject
        .iter_common_name()
        .any(|cn| cn.as_str().ok() == Some(expected_spiffe_id));
    let has_org = subject
        .iter_organization()
        .any(|org| org.as_str().ok() == Some(expected_org));
    assert!(has_cn, "Missing expected CN in certificate subject");
    assert!(has_org, "Missing expected O in certificate subject");
    assert!(
        cert.tbs_certificate.validity.not_before < cert.tbs_certificate.validity.not_after,
        "Certificate validity window is invalid"
    );
    assert!(
        is_time_within_leeway(
            cert.tbs_certificate.validity.not_before,
            cert.tbs_certificate.validity.not_after
        ),
        "Certificate is not currently valid"
    );
    assert!(
        cert.tbs_certificate
            .serial
            .to_bytes_be()
            .iter()
            .any(|byte| *byte != 0),
        "Certificate serial number should be non-zero"
    );

    let mut found_san = false;
    let mut has_spiffe_uri = false;
    for ext in cert.tbs_certificate.iter_extensions() {
        if let x509_parser::extensions::ParsedExtension::SubjectAlternativeName(san) =
            ext.parsed_extension()
        {
            found_san = true;
            let found_uri = san.general_names.iter().any(|name| {
                if let x509_parser::prelude::GeneralName::URI(uri) = name {
                    return *uri == expected_spiffe_id;
                }

                false
            });
            if found_uri {
                has_spiffe_uri = true;
                break;
            }
        }
    }
    assert!(found_san, "No SAN extension found");
    assert!(
        has_spiffe_uri,
        "Certificate SAN doesn't include expected SPIFFE ID"
    );

    cert.tbs_certificate.serial.to_bytes_be()
}

pub fn is_time_within_leeway(
    not_before: x509_parser::time::ASN1Time,
    not_after: x509_parser::time::ASN1Time,
) -> bool {
    let now = x509_parser::time::ASN1Time::now().timestamp();
    let not_before_ts = not_before.timestamp();
    let not_after_ts = not_after.timestamp();
    let now_plus_leeway = now.saturating_add(VALIDITY_LEEWAY_SECONDS);
    let now_minus_leeway = now.saturating_sub(VALIDITY_LEEWAY_SECONDS);

    not_before_ts <= now_plus_leeway && now_minus_leeway <= not_after_ts
}

pub fn assert_x509_key(path: &Path) -> Vec<u8> {
    assert!(
        path.exists(),
        "Private key file does not exist: {}",
        path.display()
    );

    let content = fs::read_to_string(path).expect("Failed to read private key");
    let key_pem = pem::parse(&content).expect("Failed to parse private key PEM");
    assert_eq!(key_pem.tag, "PRIVATE KEY", "Unexpected key PEM tag");
    assert!(!key_pem.contents.is_empty(), "Private key is empty");

    key_pem.contents
}

/// Start the mock SPIRE agent on the given socket path.
pub async fn start_mock_agent(socket_path: &PathBuf, rotation_seconds: u32) {
    if socket_path.exists() {
        fs::remove_file(socket_path).unwrap();
    }

    if let Some(parent) = socket_path.parent() {
        fs::create_dir_all(parent).unwrap();
    }

    let uds = UnixListener::bind(socket_path).expect("Failed to bind to socket");
    let uds_stream = UnixListenerStream::new(uds);

    let config = SvidConfig {
        trust_domain: "example.org".to_string(),
        workload_path: "/test/workload".to_string(),
        ttl_seconds: rotation_seconds,
    };
    let service = MockWorkloadApi::with_config(config);

    println!("starting mock agent at {:?}", socket_path);

    Server::builder()
        .add_service(SpiffeWorkloadApiServer::new(service))
        .serve_with_incoming(uds_stream)
        .await
        .unwrap();
}
