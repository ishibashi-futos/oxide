use crate::core::self_update::{
    SelfUpdateError, UpdateDecision, VersionEnv, current_target_triple, current_version,
    current_version_tag, decide_update, latest_release_info, parse_version_tag, select_asset_name,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    RunTui,
    Version,
    SelfUpdate { args: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfUpdateArgs {
    pub tag: Option<String>,
    pub prerelease: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    UnknownCommand(String),
    UnknownOption(String),
    MissingValue(String),
    InvalidVersion(String),
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
        "self-update" => Ok(Command::SelfUpdate {
            args: iter.collect(),
        }),
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
        CliError::InvalidVersion(label) => format!("invalid version: {label}"),
        CliError::UpdateFailed(message) => format!("update failed: {message}"),
    }
}

pub fn self_update_intro(env: &dyn VersionEnv, cargo_version: &str) -> String {
    let tag = current_version_tag(env, cargo_version);
    format!("self-update: current version {tag}")
}

pub fn parse_self_update_args(args: &[String]) -> Result<SelfUpdateArgs, CliError> {
    let mut tag = None;
    let mut prerelease = true;
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
            other => return Err(CliError::UnknownOption(other.to_string())),
        }
    }
    Ok(SelfUpdateArgs { tag, prerelease })
}

pub fn self_update_decision_line(
    env: &dyn VersionEnv,
    cargo_version: &str,
    args: &SelfUpdateArgs,
) -> Result<Option<String>, CliError> {
    let Some(tag) = args.tag.as_ref() else {
        return Ok(None);
    };
    let current = current_version(env, cargo_version)
        .map_err(|_| CliError::InvalidVersion("current".to_string()))?;
    let target =
        parse_version_tag(tag).map_err(|_| CliError::InvalidVersion(tag.to_string()))?;
    let decision = decide_update(&current, &target);
    let summary = match decision {
        UpdateDecision::UpdateAvailable => "update available",
        UpdateDecision::Downgrade => "downgrade",
        UpdateDecision::UpToDate => "up-to-date",
    };
    Ok(Some(format!(
        "self-update: {summary} ({current} -> {target})"
    )))
}

pub fn self_update_latest_decision_line(
    env: &dyn VersionEnv,
    cargo_version: &str,
    repo: &str,
    args: &SelfUpdateArgs,
) -> Result<Option<String>, CliError> {
    let current = current_version(env, cargo_version)
        .map_err(|_| CliError::InvalidVersion("current".to_string()))?;
    let (release, target) = latest_release_info(repo, args.prerelease).map_err(map_update_error)?;
    let decision = decide_update(&current, &target.version);
    let summary = match decision {
        UpdateDecision::UpdateAvailable => "update available",
        UpdateDecision::Downgrade => "downgrade",
        UpdateDecision::UpToDate => "up-to-date",
    };
    let mut line = format!("self-update: {summary} ({current} -> {})", target.tag);
    if let Some(triple) = current_target_triple() {
        if let Some(asset) = select_asset_name(&release, triple) {
            line.push_str(&format!(" | asset: {asset}"));
        } else {
            line.push_str(&format!(" | asset: not found for {triple}"));
        }
    } else {
        line.push_str(" | asset: unknown target");
    }
    Ok(Some(line))
}

fn map_update_error(error: SelfUpdateError) -> CliError {
    CliError::UpdateFailed(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parse_self_update_args_reads_tag() {
        let args = vec!["--tag".to_string(), "v1.2.3".to_string()];

        let parsed = parse_self_update_args(&args).unwrap();

        assert_eq!(
            parsed,
            SelfUpdateArgs {
                tag: Some("v1.2.3".to_string()),
                prerelease: true,
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
    fn self_update_decision_line_reports_update() {
        let env = FakeEnv::new("v1.0.0");
        let args = SelfUpdateArgs {
            tag: Some("v1.1.0".to_string()),
            prerelease: false,
        };

        let line = self_update_decision_line(&env, "0.1.0", &args).unwrap();

        assert_eq!(line, Some("self-update: update available (1.0.0 -> 1.1.0)".to_string()));
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
            }
        );
    }

    #[test]
    fn parse_self_update_args_defaults_prerelease_true() {
        let args = Vec::new();

        let parsed = parse_self_update_args(&args).unwrap();

        assert_eq!(
            parsed,
            SelfUpdateArgs {
                tag: None,
                prerelease: true,
            }
        );
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
