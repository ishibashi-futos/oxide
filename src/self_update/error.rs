use thiserror::Error;

#[derive(Debug, Error)]
pub enum SelfUpdateError {
    #[error("http error: {0}")]
    Http(Box<ureq::Error>),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("semver error: {0}")]
    Semver(#[from] semver::Error),
    #[error("api error: {0}")]
    ApiMessage(String),
    #[error("missing field: {0}")]
    MissingField(&'static str),
    #[error("no valid releases: {0}")]
    NoValidRelease(String),
    #[error("invalid digest: {0}")]
    InvalidDigest(String),
    #[error("digest mismatch")]
    DigestMismatch,
    #[error("missing download url")]
    MissingDownloadUrl,
    #[error("missing binary in archive: {0}")]
    MissingBinaryInArchive(String),
    #[error("release not found: {0}")]
    ReleaseNotFound(String),
    #[error("prerelease not allowed: {0}")]
    PrereleaseNotAllowed(String),
    #[error("tls configuration error: {0}")]
    TlsConfig(String),
}

impl From<ureq::Error> for SelfUpdateError {
    fn from(error: ureq::Error) -> SelfUpdateError {
        SelfUpdateError::Http(Box::new(error))
    }
}
