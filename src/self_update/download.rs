use crate::self_update::error::SelfUpdateError;
use crate::self_update::release::GitHubAsset;
use sha2::Digest;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

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
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
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
