use crate::self_update::config::SelfUpdateConfig;
use crate::self_update::download;
use crate::self_update::error::SelfUpdateError;
use crate::self_update::release::{
    GitHubAsset, GitHubRelease, ReleaseTarget, UpdateDecision, current_version, decide_update,
    fetch_releases, latest_release_info, parse_version_tag, select_release_by_tag,
    select_target_asset,
};
use crate::self_update::replace;
use crate::self_update::traits::VersionEnv;
use semver::Version;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct SelfUpdatePlan {
    pub decision: UpdateDecision,
    pub release: GitHubRelease,
    pub target: ReleaseTarget,
    pub current: Version,
}

impl SelfUpdatePlan {
    pub fn target_tag(&self) -> &str {
        &self.target.tag
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
        let current = current_version(env, cargo_version)?;
        let (release, target) = latest_release_info(&config.repo, config.allow_prerelease)?;
        let decision = decide_update(&current, &target.version);
        Ok(SelfUpdatePlan {
            decision,
            release,
            target,
            current,
        })
    }

    pub fn plan_tag(
        config: &SelfUpdateConfig,
        env: &dyn VersionEnv,
        cargo_version: &str,
        tag: &str,
    ) -> Result<SelfUpdatePlan, SelfUpdateError> {
        let current = current_version(env, cargo_version)?;
        let releases = fetch_releases(&config.repo)?;
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
        })
    }

    pub fn download_asset(asset: &GitHubAsset) -> Result<PathBuf, SelfUpdateError> {
        download::download_and_verify_asset(asset)
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
