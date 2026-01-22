use semver::Version;
use sha2::Digest;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateDecision {
    UpdateAvailable,
    Downgrade,
    UpToDate,
}

#[derive(Debug, Clone)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub prerelease: bool,
    pub draft: bool,
    pub assets: Vec<GitHubAsset>,
}

#[derive(Debug, Clone)]
pub struct GitHubAsset {
    pub name: String,
    #[allow(dead_code)]
    pub download_url: Option<String>,
    pub digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseTarget {
    pub tag: String,
    pub version: Version,
}

#[derive(Debug, Error)]
pub enum SelfUpdateError {
    #[error("http error: {0}")]
    Http(#[from] ureq::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
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
    #[allow(dead_code)]
    #[error("no backups found")]
    NoBackupFound,
}

pub fn parse_version_tag(tag: &str) -> Result<Version, semver::Error> {
    let trimmed = tag.trim();
    let normalized = trimmed.strip_prefix('v').unwrap_or(trimmed);
    Version::parse(normalized)
}

pub trait VersionEnv {
    fn get(&self, key: &str) -> Option<String>;
}

#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct SystemVersionEnv;

impl VersionEnv for SystemVersionEnv {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}

pub fn current_version_tag(env: &dyn VersionEnv, cargo_version: &str) -> String {
    env.get("OX_BUILD_VERSION")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| cargo_version.to_string())
}

pub fn current_version(
    env: &dyn VersionEnv,
    cargo_version: &str,
) -> Result<Version, semver::Error> {
    let tag = current_version_tag(env, cargo_version);
    parse_version_tag(&tag)
}

pub fn decide_update(current: &Version, target: &Version) -> UpdateDecision {
    if target > current {
        UpdateDecision::UpdateAvailable
    } else if target < current {
        UpdateDecision::Downgrade
    } else {
        UpdateDecision::UpToDate
    }
}

pub fn select_release_by_tag<'a>(
    releases: &'a [GitHubRelease],
    tag: &str,
) -> Option<GitHubRelease> {
    releases
        .iter()
        .find(|release| release.tag_name == tag)
        .cloned()
}

pub fn select_latest_release_info(
    releases: &[GitHubRelease],
    allow_prerelease: bool,
) -> Option<(GitHubRelease, ReleaseTarget)> {
    let mut best: Option<(GitHubRelease, ReleaseTarget)> = None;
    for release in releases
        .iter()
        .filter(|release| !release.draft)
        .filter(|release| allow_prerelease || !release.prerelease)
    {
        let Ok(version) = parse_version_tag(&release.tag_name) else {
            continue;
        };
        let target = ReleaseTarget {
            tag: release.tag_name.clone(),
            version,
        };
        let replace = match &best {
            None => true,
            Some((_, current)) => target.version > current.version,
        };
        if replace {
            best = Some((release.clone(), target));
        }
    }
    best
}

pub fn fetch_releases(repo: &str) -> Result<Vec<GitHubRelease>, SelfUpdateError> {
    let url = format!("https://api.github.com/repos/{repo}/releases");
    let response = ureq::get(&url)
        .set("User-Agent", "ox-self-update")
        .set("Accept", "application/vnd.github+json")
        .call()?;
    let mut body = String::new();
    response.into_reader().read_to_string(&mut body)?;
    parse_releases_json(&body)
}

pub fn parse_releases_json(body: &str) -> Result<Vec<GitHubRelease>, SelfUpdateError> {
    let json = serde_json::from_str::<serde_json::Value>(body)?;
    let Some(items) = json.as_array() else {
        if let Some(message) = json.get("message").and_then(|value| value.as_str()) {
            return Err(SelfUpdateError::ApiMessage(message.to_string()));
        }
        return Err(SelfUpdateError::ApiMessage("unexpected response".to_string()));
    };
    let mut releases = Vec::new();
    for item in items {
        let tag_name = item
            .get("tag_name")
            .and_then(|value| value.as_str())
            .ok_or(SelfUpdateError::MissingField("tag_name"))?;
        let prerelease = item
            .get("prerelease")
            .and_then(|value| value.as_bool())
            .ok_or(SelfUpdateError::MissingField("prerelease"))?;
        let draft = item
            .get("draft")
            .and_then(|value| value.as_bool())
            .ok_or(SelfUpdateError::MissingField("draft"))?;
        let mut assets = Vec::new();
        if let Some(asset_items) = item.get("assets").and_then(|value| value.as_array()) {
            for asset in asset_items {
                let Some(name) = asset.get("name").and_then(|value| value.as_str()) else {
                    continue;
                };
                let download_url = asset
                    .get("browser_download_url")
                    .and_then(|value| value.as_str())
                    .map(|value| value.to_string());
                let digest = asset
                    .get("digest")
                    .and_then(|value| value.as_str())
                    .and_then(normalize_digest);
                assets.push(GitHubAsset {
                    name: name.to_string(),
                    download_url,
                    digest,
                });
            }
        }
        releases.push(GitHubRelease {
            tag_name: tag_name.to_string(),
            prerelease,
            draft,
            assets,
        });
    }
    Ok(releases)
}

pub fn latest_release_info(
    repo: &str,
    allow_prerelease: bool,
) -> Result<(GitHubRelease, ReleaseTarget), SelfUpdateError> {
    let releases = fetch_releases(repo)?;
    select_latest_release_info(&releases, allow_prerelease)
        .ok_or_else(|| no_valid_release_error(&releases, allow_prerelease))
}

fn no_valid_release_error(
    releases: &[GitHubRelease],
    allow_prerelease: bool,
) -> SelfUpdateError {
    let has_release = releases.iter().any(|release| !release.draft);
    let has_non_prerelease = releases
        .iter()
        .any(|release| !release.draft && !release.prerelease);
    if has_release && !has_non_prerelease && !allow_prerelease {
        SelfUpdateError::NoValidRelease("only prerelease releases; use --prerelease".to_string())
    } else {
        SelfUpdateError::NoValidRelease("no matching release".to_string())
    }
}

pub fn current_target_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => Some("aarch64-pc-windows-msvc"),
        _ => None,
    }
}

pub fn select_target_asset<'a>(
    release: &'a GitHubRelease,
    target: &str,
) -> Option<&'a GitHubAsset> {
    let expected_with_tag = format!("ox-{target}-{}", release.tag_name);
    let expected_no_tag = format!("ox-{target}");
    release.assets.iter().max_by_key(|asset| {
        if asset.name == expected_with_tag || asset.name.starts_with(&expected_with_tag) {
            2
        } else if asset.name == expected_no_tag || asset.name.starts_with(&expected_no_tag) {
            1
        } else {
            0
        }
    }).and_then(|asset| {
        if asset.name == expected_with_tag
            || asset.name.starts_with(&expected_with_tag)
            || asset.name == expected_no_tag
            || asset.name.starts_with(&expected_no_tag)
        {
            Some(asset)
        } else {
            None
        }
    })
}

pub fn normalize_digest(digest: &str) -> Option<String> {
    let trimmed = digest.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

pub fn parse_sha256_digest(digest: &str) -> Result<String, SelfUpdateError> {
    let trimmed = digest.trim();
    let Some(rest) = trimmed.strip_prefix("sha256:") else {
        return Err(SelfUpdateError::InvalidDigest(trimmed.to_string()));
    };
    let hex = rest.trim();
    if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(SelfUpdateError::InvalidDigest(trimmed.to_string()));
    }
    Ok(hex.to_string())
}

pub fn compute_sha256_hex(path: &Path) -> Result<String, SelfUpdateError> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    Ok(to_hex(&digest))
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{:02x}", byte);
    }
    out
}

pub fn verify_sha256_digest(path: &Path, digest: &str) -> Result<(), SelfUpdateError> {
    let expected = parse_sha256_digest(digest)?;
    let actual = compute_sha256_hex(path)?;
    if actual == expected {
        Ok(())
    } else {
        Err(SelfUpdateError::DigestMismatch)
    }
}

pub fn download_asset_to_temp(url: &str, asset_name: &str) -> Result<PathBuf, SelfUpdateError> {
    let mut response = ureq::get(url)
        .set("User-Agent", "ox-self-update")
        .set("Accept", "application/octet-stream")
        .call()?
        .into_reader();
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SelfUpdateError::Io(std::io::Error::other("time error")))?;
    let filename = format!("ox-download-{}-{asset_name}", stamp.as_millis());
    let mut path = std::env::temp_dir();
    path.push(filename);
    let mut file = std::fs::File::create(&path)?;
    std::io::copy(&mut response, &mut file)?;
    file.flush()?;
    Ok(path)
}

pub fn download_and_verify_asset(asset: &GitHubAsset) -> Result<PathBuf, SelfUpdateError> {
    let url = asset
        .download_url
        .as_deref()
        .ok_or(SelfUpdateError::MissingDownloadUrl)?;
    let digest = asset
        .digest
        .as_deref()
        .ok_or_else(|| SelfUpdateError::InvalidDigest("missing digest".to_string()))?;
    let path = download_asset_to_temp(url, &asset.name)?;
    verify_sha256_digest(&path, digest)?;
    unpack_if_needed(&path, &asset.name)
}

fn unpack_if_needed(path: &Path, asset_name: &str) -> Result<PathBuf, SelfUpdateError> {
    if is_tar_gz(asset_name) {
        extract_tar_gz(path, asset_name)
    } else {
        Ok(path.to_path_buf())
    }
}

fn is_tar_gz(name: &str) -> bool {
    name.ends_with(".tar.gz") || name.ends_with(".tgz")
}

fn extract_tar_gz(path: &Path, asset_name: &str) -> Result<PathBuf, SelfUpdateError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SelfUpdateError::Io(std::io::Error::other("time error")))?;
    let mut dir = std::env::temp_dir();
    let safe_name = asset_name.replace('/', "_");
    dir.push(format!("ox-extract-{}-{}", stamp.as_millis(), safe_name));
    std::fs::create_dir_all(&dir)?;
    extract_tar_gz_to(path, &dir)?;
    find_binary_in_dir(&dir)
        .ok_or_else(|| SelfUpdateError::MissingBinaryInArchive(asset_name.to_string()))
}

fn extract_tar_gz_to(path: &Path, dir: &Path) -> Result<(), SelfUpdateError> {
    let file = std::fs::File::open(path)?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dir)?;
    Ok(())
}

fn find_binary_in_dir(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let entries = std::fs::read_dir(&next).ok()?;
        for entry in entries {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if is_binary_name(&path) {
                return Some(path);
            }
        }
    }
    None
}

fn is_binary_name(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    #[cfg(windows)]
    {
        name == "ox.exe"
    }
    #[cfg(not(windows))]
    {
        name == "ox"
    }
}

pub fn backup_path_for(current_exe: &Path, version_tag: &str) -> PathBuf {
    let name = format!("ox-{}", version_tag);
    current_exe
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}

pub fn replace_current_exe(downloaded: &Path, version_tag: &str) -> Result<PathBuf, SelfUpdateError> {
    let current_exe = std::env::current_exe()?;
    let backup = backup_path_for(&current_exe, version_tag);
    if !backup.exists() {
        std::fs::copy(&current_exe, &backup)?;
    }
    let temp = current_exe.with_extension("new");
    std::fs::copy(downloaded, &temp)?;
    #[cfg(unix)]
    {
        let perms = std::fs::metadata(&current_exe)?.permissions();
        std::fs::set_permissions(&temp, perms)?;
    }
    std::fs::rename(&temp, &current_exe)?;
    Ok(backup)
}

pub fn list_backups(current_exe: &Path) -> Result<Vec<PathBuf>, SelfUpdateError> {
    let dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
    let mut backups = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.starts_with("ox-") {
            backups.push(path);
        }
    }
    backups.sort();
    Ok(backups)
}

pub fn rollback_named(backup: &Path) -> Result<PathBuf, SelfUpdateError> {
    let current_exe = std::env::current_exe()?;
    std::fs::copy(backup, &current_exe)?;
    Ok(backup.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_uses_build_version_when_set() {
        let env = FakeEnv::new("v2.0.0");

        let current = current_version(&env, "0.1.0").unwrap();

        assert_eq!(current, Version::new(2, 0, 0));
    }

    #[test]
    fn current_version_falls_back_to_cargo_version() {
        let env = FakeEnv::empty();

        let current = current_version(&env, "0.1.0").unwrap();

        assert_eq!(current, Version::new(0, 1, 0));
    }

    #[test]
    fn current_version_ignores_empty_build_version() {
        let env = FakeEnv::new("  ");

        let current = current_version(&env, "0.1.0").unwrap();

        assert_eq!(current, Version::new(0, 1, 0));
    }

    #[test]
    fn parse_version_tag_accepts_v_prefix() {
        let version = parse_version_tag("v1.2.3").unwrap();

        assert_eq!(version, Version::new(1, 2, 3));
    }

    #[test]
    fn parse_version_tag_accepts_plain_version() {
        let version = parse_version_tag("1.2.3").unwrap();

        assert_eq!(version, Version::new(1, 2, 3));
    }

    #[test]
    fn parse_version_tag_accepts_prerelease() {
        let version = parse_version_tag("v1.2.3-alpha.1").unwrap();

        assert_eq!(version.to_string(), "1.2.3-alpha.1");
    }

    #[test]
    fn decide_update_returns_update_available_when_target_is_newer() {
        let current = Version::new(1, 0, 0);
        let target = Version::new(1, 1, 0);

        assert_eq!(decide_update(&current, &target), UpdateDecision::UpdateAvailable);
    }

    #[test]
    fn decide_update_returns_downgrade_when_target_is_older() {
        let current = Version::new(1, 1, 0);
        let target = Version::new(1, 0, 0);

        assert_eq!(decide_update(&current, &target), UpdateDecision::Downgrade);
    }

    #[test]
    fn decide_update_returns_up_to_date_when_versions_match() {
        let current = Version::new(1, 1, 0);
        let target = Version::new(1, 1, 0);

        assert_eq!(decide_update(&current, &target), UpdateDecision::UpToDate);
    }

    #[test]
    fn normalize_digest_strips_whitespace() {
        let digest = normalize_digest(" sha256:abcd ");

        assert_eq!(digest, Some("sha256:abcd".to_string()));
    }

    #[test]
    fn parse_sha256_digest_accepts_valid() {
        let digest = parse_sha256_digest("sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
            .unwrap();

        assert_eq!(
            digest,
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string()
        );
    }

    #[test]
    fn unpack_if_needed_extracts_tar_gz_binary() {
        let temp = tempfile::tempdir().unwrap();
        let archive_path = temp.path().join("ox-aarch64-apple-darwin.tar.gz");
        let payload_dir = temp.path().join("payload");
        std::fs::create_dir_all(&payload_dir).unwrap();
        let binary_path = payload_dir.join("ox");
        std::fs::write(&binary_path, b"fake").unwrap();
        let tar_gz = std::fs::File::create(&archive_path).unwrap();
        let encoder = flate2::write::GzEncoder::new(tar_gz, flate2::Compression::default());
        let mut archive = tar::Builder::new(encoder);
        archive.append_path_with_name(&binary_path, "ox").unwrap();
        let encoder = archive.into_inner().unwrap();
        encoder.finish().unwrap();

        let extracted = unpack_if_needed(&archive_path, "ox-aarch64-apple-darwin.tar.gz").unwrap();

        assert!(extracted.ends_with("ox"));
        assert!(extracted.exists());
    }

    #[test]
    fn parse_sha256_digest_rejects_invalid() {
        let error = parse_sha256_digest("sha1:abc").unwrap_err();

        assert!(matches!(error, SelfUpdateError::InvalidDigest(_)));
    }

    #[test]
    fn verify_sha256_digest_matches_file() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("file.txt");
        std::fs::write(&path, b"hello").unwrap();
        let digest = format!("sha256:{}", compute_sha256_hex(&path).unwrap());

        let result = verify_sha256_digest(&path, &digest);

        assert!(result.is_ok());
    }

    #[test]
    fn select_release_by_tag_finds_match() {
        let releases = vec![
            GitHubRelease {
                tag_name: "v0.1.0".to_string(),
                prerelease: true,
                draft: false,
                assets: Vec::new(),
            },
            GitHubRelease {
                tag_name: "v0.2.0".to_string(),
                prerelease: true,
                draft: false,
                assets: Vec::new(),
            },
        ];

        let release = select_release_by_tag(&releases, "v0.2.0").unwrap();

        assert_eq!(release.tag_name, "v0.2.0");
    }

    #[test]
    fn parse_releases_json_reports_api_message() {
        let body = r#"{"message":"API rate limit exceeded"}"#;

        let error = parse_releases_json(body).unwrap_err();

        assert!(matches!(error, SelfUpdateError::ApiMessage(_)));
    }

    #[test]
    fn latest_release_info_reports_prerelease_only() {
        let releases = vec![GitHubRelease {
            tag_name: "v1.0.0-alpha.1".to_string(),
            prerelease: true,
            draft: false,
            assets: Vec::new(),
        }];

        let result = select_latest_release_info(&releases, false);

        assert!(result.is_none());
    }

    #[test]
    fn no_valid_release_error_explains_prerelease_only() {
        let releases = vec![GitHubRelease {
            tag_name: "v1.0.0-alpha.1".to_string(),
            prerelease: true,
            draft: false,
            assets: Vec::new(),
        }];

        let error = no_valid_release_error(&releases, false);

        let message = error.to_string();
        assert!(message.contains("only prerelease releases"));
    }

    #[derive(Debug)]
    struct FakeEnv {
        value: Option<String>,
    }

    impl FakeEnv {
        fn new(value: &str) -> Self {
            Self {
                value: Some(value.to_string()),
            }
        }

        fn empty() -> Self {
            Self { value: None }
        }
    }

    impl VersionEnv for FakeEnv {
        fn get(&self, key: &str) -> Option<String> {
            if key == "OX_BUILD_VERSION" {
                self.value.clone()
            } else {
                None
            }
        }
    }
}
