use crate::self_update::{
    GitHubAsset, SelfUpdateConfig, SelfUpdateError, SelfUpdatePlan, SelfUpdateService,
    UpdateDecision, VersionEnv, current_target_triple, current_version_tag,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    RunTui,
    Version,
    SelfUpdate { args: Vec<String> },
    SelfUpdateRollback { yes: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfUpdateArgs {
    pub tag: Option<String>,
    pub prerelease: bool,
    pub yes: bool,
    pub insecure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    UnknownCommand(String),
    UnknownOption(String),
    MissingValue(String),
    UpdateFailed(String),
}

pub fn parse_args<I>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = String>,
{
    let mut iter = args.into_iter();
    let _ = iter.next();
    let Some(first) = iter.next() else {
        return Ok(Command::RunTui);
    };
    match first.as_str() {
        "--version" | "-V" => Ok(Command::Version),
        "self-update" => {
            let args: Vec<String> = iter.collect();
            if args.first().map(|value| value.as_str()) == Some("rollback") {
                let yes = args.iter().any(|value| value == "--yes" || value == "-y");
                Ok(Command::SelfUpdateRollback { yes })
            } else {
                Ok(Command::SelfUpdate { args })
            }
        }
        other => Err(CliError::UnknownCommand(other.to_string())),
    }
}

pub fn usage() -> &'static str {
    "Usage:\n  ox\n  ox --version\n  ox self-update"
}

pub fn render_error(error: &CliError) -> String {
    match error {
        CliError::UnknownCommand(command) => format!("unknown command: {command}"),
        CliError::UnknownOption(option) => format!("unknown option: {option}"),
        CliError::MissingValue(option) => format!("missing value for {option}"),
        CliError::UpdateFailed(message) => format!("update failed: {message}"),
    }
}

pub fn self_update_intro(env: &dyn VersionEnv, cargo_version: &str) -> String {
    let tag = current_version_tag(env, cargo_version);
    format!("self-update: current version {tag}")
}

fn build_version_tag() -> Option<&'static str> {
    option_env!("OX_BUILD_VERSION").and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn version_tag_for_output(
    env: &dyn VersionEnv,
    cargo_version: &str,
    build_tag: Option<&str>,
) -> String {
    if let Some(tag) = build_tag {
        return tag.to_string();
    }
    current_version_tag(env, cargo_version)
}

pub fn version_line(env: &dyn VersionEnv, cargo_version: &str) -> String {
    let tag = version_tag_for_output(env, cargo_version, build_version_tag());
    format!("ox {tag}")
}

pub fn parse_self_update_args(args: &[String]) -> Result<SelfUpdateArgs, CliError> {
    let mut tag = None;
    let mut prerelease = false;
    let mut yes = false;
    let mut insecure = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--tag" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue("--tag".to_string()))?;
                tag = Some(value.to_string());
            }
            "--prerelease" => {
                prerelease = true;
            }
            "--yes" | "-y" => {
                yes = true;
            }
            "--insecure" => {
                insecure = true;
            }
            other => return Err(CliError::UnknownOption(other.to_string())),
        }
    }
    Ok(SelfUpdateArgs {
        tag,
        prerelease,
        yes,
        insecure,
    })
}

#[derive(Debug)]
pub struct SelfUpdatePlanSummary {
    pub line: String,
    pub decision: UpdateDecision,
    pub asset: Option<GitHubAsset>,
    pub current_tag: String,
}

pub fn self_update_latest_plan(
    env: &dyn VersionEnv,
    cargo_version: &str,
    config: &SelfUpdateConfig,
) -> Result<SelfUpdatePlanSummary, CliError> {
    let plan =
        SelfUpdateService::plan_latest(config, env, cargo_version).map_err(map_update_error)?;
    Ok(build_plan_summary(plan))
}

fn map_update_error(error: SelfUpdateError) -> CliError {
    CliError::UpdateFailed(error.to_string())
}

fn digest_status(digest: Option<&str>) -> String {
    match digest {
        Some(value) => value.to_string(),
        None => "missing digest".to_string(),
    }
}

fn build_plan_summary(plan: SelfUpdatePlan) -> SelfUpdatePlanSummary {
    let summary = match plan.decision {
        UpdateDecision::UpdateAvailable => "update available",
        UpdateDecision::Downgrade => "downgrade",
        UpdateDecision::UpToDate => "up-to-date",
    };
    let mut line = format!(
        "self-update: {summary} ({} -> {})",
        plan.current,
        plan.target_tag()
    );
    let mut asset = None;
    if let Some(triple) = current_target_triple() {
        match plan.asset_for_target(triple) {
            Some(found) => {
                line.push_str(&format!(" | asset: {}", found.name));
                line.push_str(&format!(
                    " | digest: {}",
                    digest_status(found.digest.as_deref())
                ));
                asset = Some(found.clone());
            }
            None => {
                line.push_str(&format!(" | asset: not found for {triple}"));
            }
        }
    } else {
        line.push_str(" | asset: unknown target");
    }
    SelfUpdatePlanSummary {
        line,
        decision: plan.decision,
        asset,
        current_tag: plan.current_tag().to_string(),
    }
}

pub fn self_update_tag_plan(
    env: &dyn VersionEnv,
    cargo_version: &str,
    config: &SelfUpdateConfig,
    tag: &str,
) -> Result<SelfUpdatePlanSummary, CliError> {
    let plan =
        SelfUpdateService::plan_tag(config, env, cargo_version, tag).map_err(map_update_error)?;
    Ok(build_plan_summary(plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_update::release::{GitHubRelease, ReleaseTarget};
    use semver::Version;

    #[test]
    fn parse_args_defaults_to_tui() {
        let command = parse_args(vec!["ox".to_string()]).unwrap();

        assert_eq!(command, Command::RunTui);
    }

    #[test]
    fn parse_args_reads_version() {
        let command = parse_args(vec!["ox".to_string(), "--version".to_string()]).unwrap();

        assert_eq!(command, Command::Version);
    }

    #[test]
    fn parse_args_reads_self_update_with_args() {
        let command = parse_args(vec![
            "ox".to_string(),
            "self-update".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::SelfUpdate {
                args: vec!["--yes".to_string()]
            }
        );
    }

    #[test]
    fn parse_args_reads_self_update_rollback() {
        let command = parse_args(vec![
            "ox".to_string(),
            "self-update".to_string(),
            "rollback".to_string(),
        ])
        .unwrap();

        assert_eq!(command, Command::SelfUpdateRollback { yes: false });
    }

    #[test]
    fn parse_args_reads_self_update_rollback_yes() {
        let command = parse_args(vec![
            "ox".to_string(),
            "self-update".to_string(),
            "rollback".to_string(),
            "--yes".to_string(),
        ])
        .unwrap();

        assert_eq!(command, Command::SelfUpdateRollback { yes: true });
    }

    #[test]
    fn parse_args_rejects_unknown_command() {
        let error = parse_args(vec!["ox".to_string(), "nope".to_string()]).unwrap_err();

        assert_eq!(error, CliError::UnknownCommand("nope".to_string()));
    }

    #[test]
    fn render_error_formats_update_failed() {
        let error = CliError::UpdateFailed("rate limited".to_string());

        let message = render_error(&error);

        assert_eq!(message, "update failed: rate limited");
    }

    #[test]
    fn digest_status_reports_missing_digest() {
        let status = digest_status(None);

        assert_eq!(status, "missing digest");
    }

    #[test]
    fn self_update_intro_uses_build_version() {
        let env = FakeEnv::new("v1.2.3");

        let message = self_update_intro(&env, "0.1.0");

        assert_eq!(message, "self-update: current version v1.2.3");
    }

    #[test]
    fn self_update_intro_falls_back_to_cargo_version() {
        let env = FakeEnv::empty();

        let message = self_update_intro(&env, "0.1.0");

        assert_eq!(message, "self-update: current version 0.1.0");
    }

    #[test]
    fn version_line_uses_build_version() {
        let env = FakeEnv::new("v9.9.9");

        let message = version_tag_for_output(&env, "0.1.0", Some("v9.9.9"));

        assert_eq!(message, "v9.9.9");
    }

    #[test]
    fn version_line_falls_back_to_cargo_version() {
        let env = FakeEnv::empty();

        let message = version_tag_for_output(&env, "0.1.0", None);

        assert_eq!(message, "0.1.0");
    }

    #[test]
    fn version_line_prefers_build_version_over_runtime() {
        let env = FakeEnv::new("v1.2.3");

        let message = version_tag_for_output(&env, "0.1.0", Some("v9.9.9"));

        assert_eq!(message, "v9.9.9");
    }

    #[test]
    fn parse_self_update_args_reads_tag() {
        let args = vec!["--tag".to_string(), "v1.2.3".to_string()];

        let parsed = parse_self_update_args(&args).unwrap();

        assert_eq!(
            parsed,
            SelfUpdateArgs {
                tag: Some("v1.2.3".to_string()),
                prerelease: false,
                yes: false,
                insecure: false,
            }
        );
    }

    #[test]
    fn parse_self_update_args_rejects_missing_tag_value() {
        let args = vec!["--tag".to_string()];

        let error = parse_self_update_args(&args).unwrap_err();

        assert_eq!(error, CliError::MissingValue("--tag".to_string()));
    }

    #[test]
    fn parse_self_update_args_accepts_prerelease() {
        let args = vec!["--prerelease".to_string()];

        let parsed = parse_self_update_args(&args).unwrap();

        assert_eq!(
            parsed,
            SelfUpdateArgs {
                tag: None,
                prerelease: true,
                yes: false,
                insecure: false,
            }
        );
    }

    #[test]
    fn parse_self_update_args_defaults_prerelease_false() {
        let args = Vec::new();

        let parsed = parse_self_update_args(&args).unwrap();

        assert_eq!(
            parsed,
            SelfUpdateArgs {
                tag: None,
                prerelease: false,
                yes: false,
                insecure: false,
            }
        );
    }

    #[test]
    fn parse_self_update_args_accepts_yes() {
        let args = vec!["--yes".to_string()];

        let parsed = parse_self_update_args(&args).unwrap();

        assert_eq!(
            parsed,
            SelfUpdateArgs {
                tag: None,
                prerelease: false,
                yes: true,
                insecure: false,
            }
        );
    }

    #[test]
    fn parse_self_update_args_accepts_insecure() {
        let args = vec!["--insecure".to_string()];

        let parsed = parse_self_update_args(&args).unwrap();

        assert_eq!(
            parsed,
            SelfUpdateArgs {
                tag: None,
                prerelease: false,
                yes: false,
                insecure: true,
            }
        );
    }

    #[test]
    fn build_plan_summary_carries_current_tag() {
        let plan = SelfUpdatePlan {
            decision: UpdateDecision::UpdateAvailable,
            release: GitHubRelease {
                tag_name: "v0.4.2".to_string(),
                prerelease: false,
                draft: false,
                assets: Vec::new(),
            },
            target: ReleaseTarget {
                tag: "v0.4.2".to_string(),
                version: Version::parse("0.4.2").expect("version"),
            },
            current: Version::parse("0.3.0").expect("version"),
            current_tag: "v0.3.0".to_string(),
        };

        let summary = build_plan_summary(plan);

        assert_eq!(summary.current_tag, "v0.3.0");
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
