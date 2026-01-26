use crate::self_update::{
    config::SelfUpdateConfig,
    download,
    error::SelfUpdateError,
    http::HttpClient,
    release::{
        GitHubAsset, GitHubRelease, ReleaseTarget, UpdateDecision, decide_update, fetch_releases,
        latest_release_info, parse_version_tag, select_release_by_tag, select_target_asset,
    },
    replace,
    traits::VersionEnv,
};
use semver::Version;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct SelfUpdatePlan {
    pub decision: UpdateDecision,
    pub release: GitHubRelease,
    pub target: ReleaseTarget,
    pub current: Version,
    pub current_tag: String,
}

impl SelfUpdatePlan {
    pub fn target_tag(&self) -> &str {
        &self.target.tag
    }

    pub fn current_tag(&self) -> &str {
        &self.current_tag
    }

    pub fn asset_for_target(&self, target: &str) -> Option<&GitHubAsset> {
        select_target_asset(&self.release, target)
    }
}

pub struct SelfUpdateService;

impl SelfUpdateService {
    pub fn plan_latest(
        config: &SelfUpdateConfig,
        env: &dyn VersionEnv,
        cargo_version: &str,
    ) -> Result<SelfUpdatePlan, SelfUpdateError> {
        let current_tag = crate::self_update::release::current_version_tag(env, cargo_version);
        let current = parse_version_tag(&current_tag)?;
        let client = HttpClient::new(config.allow_insecure)?;
        let (release, target) =
            latest_release_info(client.agent(), &config.repo, config.allow_prerelease)?;
        let decision = decide_update(&current, &target.version);
        Ok(SelfUpdatePlan {
            decision,
            release,
            target,
            current,
            current_tag,
        })
    }

    pub fn plan_tag(
        config: &SelfUpdateConfig,
        env: &dyn VersionEnv,
        cargo_version: &str,
        tag: &str,
    ) -> Result<SelfUpdatePlan, SelfUpdateError> {
        let current_tag = crate::self_update::release::current_version_tag(env, cargo_version);
        let current = parse_version_tag(&current_tag)?;
        let client = HttpClient::new(config.allow_insecure)?;
        let releases = fetch_releases(client.agent(), &config.repo)?;
        let release = select_release_by_tag(&releases, tag)
            .ok_or_else(|| SelfUpdateError::ReleaseNotFound(tag.to_string()))?;
        if release.prerelease && !config.allow_prerelease {
            return Err(SelfUpdateError::PrereleaseNotAllowed(tag.to_string()));
        }
        let version = parse_version_tag(&release.tag_name)?;
        let target = ReleaseTarget {
            tag: release.tag_name.clone(),
            version,
        };
        let decision = decide_update(&current, &target.version);
        Ok(SelfUpdatePlan {
            decision,
            release,
            target,
            current,
            current_tag,
        })
    }

    pub fn download_asset(
        asset: &GitHubAsset,
        config: &SelfUpdateConfig,
    ) -> Result<PathBuf, SelfUpdateError> {
        let client = HttpClient::new(config.allow_insecure)?;
        download::download_and_verify_asset(client.agent(), asset)
    }

    pub fn replace_current(
        downloaded: &Path,
        target_tag: &str,
    ) -> Result<PathBuf, SelfUpdateError> {
        replace::replace_current_exe(downloaded, target_tag)
    }

    pub fn list_backups() -> Result<Vec<PathBuf>, SelfUpdateError> {
        replace::list_backups()
    }

    pub fn rollback(backup: &Path) -> Result<PathBuf, SelfUpdateError> {
        replace::rollback_named(backup)
    }
}
