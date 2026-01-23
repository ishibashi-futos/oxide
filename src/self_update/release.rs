use crate::self_update::error::SelfUpdateError;
use crate::self_update::traits::VersionEnv;
use semver::Version;
use std::io::Read;
use ureq::Agent;

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

pub fn parse_version_tag(tag: &str) -> Result<Version, semver::Error> {
    let trimmed = tag.trim();
    let normalized = trimmed.strip_prefix('v').unwrap_or(trimmed);
    Version::parse(normalized)
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

pub fn select_release_by_tag(releases: &[GitHubRelease], tag: &str) -> Option<GitHubRelease> {
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

pub fn fetch_releases(client: &Agent, repo: &str) -> Result<Vec<GitHubRelease>, SelfUpdateError> {
    let url = format!("https://api.github.com/repos/{repo}/releases");
    let response = client
        .get(&url)
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
        return Err(SelfUpdateError::ApiMessage(
            "unexpected response".to_string(),
        ));
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

fn normalize_digest(digest: &str) -> Option<String> {
    let trimmed = digest.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

pub fn latest_release_info(
    client: &Agent,
    repo: &str,
    allow_prerelease: bool,
) -> Result<(GitHubRelease, ReleaseTarget), SelfUpdateError> {
    let releases = fetch_releases(client, repo)?;
    select_latest_release_info(&releases, allow_prerelease)
        .ok_or_else(|| no_valid_release_error(&releases, allow_prerelease))
}

fn no_valid_release_error(releases: &[GitHubRelease], allow_prerelease: bool) -> SelfUpdateError {
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
    release
        .assets
        .iter()
        .max_by_key(|asset| {
            if asset.name == expected_with_tag || asset.name.starts_with(&expected_with_tag) {
                2
            } else if asset.name == expected_no_tag || asset.name.starts_with(&expected_no_tag) {
                1
            } else {
                0
            }
        })
        .and_then(|asset| {
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
