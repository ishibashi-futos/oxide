use thiserror::Error;

#[derive(Debug, Error)]
pub enum SelfUpdateError {
    #[error("http error: {0}")]
    Http(#[from] ureq::Error),
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
}
