//! PEM certificates as the DER root a client trusts.

use rustls::pki_types::CertificateDer;
use rustls::pki_types::pem::PemObject;

use crate::Error;

/// The first `CERTIFICATE` section of `pem` as DER.
///
/// # Errors
/// `InvalidConfig` when `pem` has no certificate section, the section is not
/// valid PEM, or its bytes are not a certificate rustls accepts as a root.
pub fn pem_to_der(pem: &[u8]) -> Result<Vec<u8>, Error> {
  let cert = CertificateDer::from_pem_slice(pem)
    .map_err(|err| Error::InvalidConfig(format!("invalid PEM certificate: {err}")))?;
  rustls::RootCertStore::empty()
    .add(cert.clone())
    .map_err(|err| Error::InvalidConfig(format!("invalid root certificate: {err}")))?;
  Ok(cert.as_ref().to_vec())
}

/// The first certificate of the PEM file at `path`, as DER.
///
/// # Errors
/// `InvalidConfig` when the file cannot be read or `pem_to_der` refuses it.
pub fn pem_file_to_der(path: &str) -> Result<Vec<u8>, Error> {
  let pem = std::fs::read(path)
    .map_err(|err| Error::InvalidConfig(format!("cannot read certificate file {path}: {err}")))?;
  pem_to_der(&pem)
}

#[cfg(test)]
#[path = "tests/pem_test.rs"]
mod tests;
