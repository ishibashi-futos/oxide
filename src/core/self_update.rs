use semver::Version;
use std::io::Read;
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

pub fn select_latest_release(
    releases: &[GitHubRelease],
    allow_prerelease: bool,
) -> Option<ReleaseTarget> {
    releases
        .iter()
        .filter(|release| !release.draft)
        .filter(|release| allow_prerelease || !release.prerelease)
        .filter_map(|release| {
            let version = parse_version_tag(&release.tag_name).ok()?;
            Some(ReleaseTarget {
                tag: release.tag_name.clone(),
                version,
            })
        })
        .max_by(|left, right| left.version.cmp(&right.version))
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

pub fn select_asset_name(release: &GitHubRelease, target: &str) -> Option<String> {
    select_target_asset(release, target).map(|asset| asset.name.clone())
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
    fn select_latest_release_ignores_draft_and_invalid_tags() {
        let releases = vec![
            GitHubRelease {
                tag_name: "nightly".to_string(),
                prerelease: false,
                draft: false,
                assets: Vec::new(),
            },
            GitHubRelease {
                tag_name: "v1.0.0".to_string(),
                prerelease: false,
                draft: true,
                assets: Vec::new(),
            },
            GitHubRelease {
                tag_name: "v1.1.0".to_string(),
                prerelease: false,
                draft: false,
                assets: Vec::new(),
            },
        ];

        let latest = select_latest_release(&releases, false).unwrap();

        assert_eq!(latest.tag, "v1.1.0");
        assert_eq!(latest.version, Version::new(1, 1, 0));
    }

    #[test]
    fn select_latest_release_skips_prerelease_by_default() {
        let releases = vec![
            GitHubRelease {
                tag_name: "v1.1.0".to_string(),
                prerelease: false,
                draft: false,
                assets: Vec::new(),
            },
            GitHubRelease {
                tag_name: "v2.0.0-alpha.1".to_string(),
                prerelease: true,
                draft: false,
                assets: Vec::new(),
            },
        ];

        let latest = select_latest_release(&releases, false).unwrap();

        assert_eq!(latest.tag, "v1.1.0");
    }

    #[test]
    fn select_latest_release_allows_prerelease_when_flag_set() {
        let releases = vec![
            GitHubRelease {
                tag_name: "v1.1.0".to_string(),
                prerelease: false,
                draft: false,
                assets: Vec::new(),
            },
            GitHubRelease {
                tag_name: "v2.0.0-alpha.1".to_string(),
                prerelease: true,
                draft: false,
                assets: Vec::new(),
            },
        ];

        let latest = select_latest_release(&releases, true).unwrap();

        assert_eq!(latest.tag, "v2.0.0-alpha.1");
    }

    #[test]
    fn select_asset_name_finds_matching_asset() {
        let release = GitHubRelease {
            tag_name: "v1.2.3".to_string(),
            prerelease: false,
            draft: false,
            assets: vec![
                GitHubAsset {
                    name: "ox-x86_64-unknown-linux-gnu-v1.2.3".to_string(),
                    download_url: None,
                    digest: None,
                },
                GitHubAsset {
                    name: "readme.txt".to_string(),
                    download_url: None,
                    digest: None,
                },
            ],
        };

        let asset = select_asset_name(&release, "x86_64-unknown-linux-gnu").unwrap();

        assert_eq!(asset, "ox-x86_64-unknown-linux-gnu-v1.2.3");
    }

    #[test]
    fn select_asset_name_falls_back_to_target_only() {
        let release = GitHubRelease {
            tag_name: "v1.2.3".to_string(),
            prerelease: false,
            draft: false,
            assets: vec![
                GitHubAsset {
                    name: "ox-x86_64-unknown-linux-gnu.tar.gz".to_string(),
                    download_url: None,
                    digest: None,
                },
                GitHubAsset {
                    name: "readme.txt".to_string(),
                    download_url: None,
                    digest: None,
                },
            ],
        };

        let asset = select_asset_name(&release, "x86_64-unknown-linux-gnu").unwrap();

        assert_eq!(asset, "ox-x86_64-unknown-linux-gnu.tar.gz");
    }

    #[test]
    fn normalize_digest_strips_whitespace() {
        let digest = normalize_digest(" sha256:abcd ");

        assert_eq!(digest, Some("sha256:abcd".to_string()));
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
