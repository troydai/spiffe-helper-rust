/* The file_system module abstract the interaction of this program with the FileSystem */

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{fs, path::PathBuf, str::FromStr};

use anyhow::{anyhow, Context, Result};
use spiffe::bundle::x509::X509Bundle;
use spiffe::cert::Certificate;

use crate::cli::Config;

pub trait X509CertsWriter {
    fn write_certs(&self, certificates: &[Certificate]) -> Result<()>;
    fn write_key(&self, key: &[u8]) -> Result<()>;
    fn write_bundle(&self, bundle: &X509Bundle) -> Result<()>;
}

#[derive(Debug)]
pub struct LocalFileSystem {
    output_dir: PathBuf, // from the cert_dir in the config
    cer_path: PathBuf,
    key_path: PathBuf,
    bundle_path: PathBuf,
    cert_mode: u32,
    key_mode: u32,
    bundle_mode: u32,
}

impl LocalFileSystem {
    pub fn new(config: &Config) -> Result<Self> {
        let cert_dir = config
            .cert_dir
            .as_ref()
            .ok_or_else(|| anyhow!("cert_dir must be configured"))?;

        let output_dir = PathBuf::from_str(cert_dir).with_context(|| {
            format!(
                "Failed create path from specified directory path: {}",
                cert_dir
            )
        })?;

        Ok(Self {
            output_dir: output_dir.clone(),
            cer_path: output_dir.join(config.svid_file_name()),
            key_path: output_dir.join(config.svid_key_file_name()),
            bundle_path: output_dir.join(config.svid_bundle_file_name()),
            cert_mode: config.cert_file_mode(),
            key_mode: config.key_file_mode(),
            bundle_mode: config.cert_file_mode(),
        })
    }

    pub fn ensure(self) -> Result<Self> {
        if !&self.output_dir.exists() {
            fs::create_dir_all(&self.output_dir).with_context(|| {
                format!(
                    "Failed to create output directory: {}",
                    self.output_dir.display()
                )
            })?;
        }

        Ok(self)
    }
}

impl X509CertsWriter for LocalFileSystem {
    fn write_certs(&self, certificates: &[Certificate]) -> Result<()> {
        let content = certificates
            .iter()
            .map(|c| {
                pem::encode(&pem::Pem {
                    tag: "CERTIFICATE".to_string(),
                    contents: c.as_ref().to_vec(),
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(&self.cer_path, content).with_context(|| {
            format!("Failed to write certificate to {}", self.cer_path.display())
        })?;

        #[cfg(unix)]
        fs::set_permissions(&self.cer_path, fs::Permissions::from_mode(self.cert_mode))
            .with_context(|| {
                format!(
                    "Failed to set permissions on certificate file {}",
                    self.cer_path.display()
                )
            })?;

        Ok(())
    }

    fn write_key(&self, key: &[u8]) -> Result<()> {
        let key_pem = pem::Pem {
            tag: "PRIVATE KEY".to_string(),
            contents: Vec::from(key),
        };

        let content = pem::encode(&key_pem);

        fs::write(&self.key_path, content)
            .with_context(|| format!("Failed to write key to {}", self.key_path.display()))?;

        #[cfg(unix)]
        fs::set_permissions(&self.key_path, fs::Permissions::from_mode(self.key_mode))
            .with_context(|| {
                format!(
                    "Failed to set permissions on private key file {}",
                    self.key_path.display()
                )
            })?;

        Ok(())
    }

    fn write_bundle(&self, bundle: &X509Bundle) -> Result<()> {
        let bundle_pem = bundle
            .authorities()
            .iter()
            .map(|cert: &spiffe::cert::Certificate| {
                pem::encode(&pem::Pem {
                    tag: "CERTIFICATE".to_string(),
                    contents: cert.as_ref().to_vec(),
                })
            })
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(&self.bundle_path, bundle_pem)
            .with_context(|| format!("Failed to write bundle to {}", self.bundle_path.display()))?;

        #[cfg(unix)]
        fs::set_permissions(
            &self.bundle_path,
            fs::Permissions::from_mode(self.bundle_mode),
        )
        .with_context(|| {
            format!(
                "Failed to set permissions on bundle file {}",
                self.bundle_path.display()
            )
        })?;

        Ok(())
    }
}
