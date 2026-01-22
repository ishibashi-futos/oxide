mod app;
mod cli;
mod config;
mod core;
mod error;
mod opener;
mod tabs;
mod ui;

use crate::{app::App, error::AppResult, opener::PlatformOpener};
use crate::cli::{
    Command, parse_args, parse_self_update_args, render_error, self_update_intro,
    self_update_latest_plan, self_update_tag_plan, usage,
};
use crate::core::self_update::{download_and_verify_asset, replace_current_exe};
use crate::core::self_update::SystemVersionEnv;

fn main() -> AppResult<()> {
    let command = match parse_args(std::env::args()) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("error: {}", render_error(&error));
            eprintln!("{}", usage());
            return Ok(());
        }
    };
    match command {
        Command::RunTui => {
            let current_dir = std::env::current_dir()?;
            let app = App::load(current_dir)?;
            let opener = PlatformOpener;
            ui::run(app, &opener)
        }
        Command::Version => {
            println!("ox {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::SelfUpdate { args } => {
            let env = SystemVersionEnv;
            println!("{}", self_update_intro(&env, env!("CARGO_PKG_VERSION")));
            match parse_self_update_args(&args) {
                Ok(parsed) => {
                    let plan = if let Some(tag) = parsed.tag.as_ref() {
                        match self_update_tag_plan(
                            &env,
                            env!("CARGO_PKG_VERSION"),
                            "ishibashi-futos/oxide",
                            tag,
                            parsed.prerelease,
                        ) {
                            Ok(plan) => plan,
                            Err(error) => {
                                eprintln!("error: {}", render_error(&error));
                                return Ok(());
                            }
                        }
                    } else {
                        match self_update_latest_plan(
                            &env,
                            env!("CARGO_PKG_VERSION"),
                            "ishibashi-futos/oxide",
                            &parsed,
                        ) {
                            Ok(plan) => plan,
                            Err(error) => {
                                eprintln!("error: {}", render_error(&error));
                                return Ok(());
                            }
                        }
                    };
                    println!("{}", plan.line);
                    if matches!(plan.decision, crate::core::self_update::UpdateDecision::UpToDate) {
                        return Ok(());
                    }
                    if plan.asset.is_none() {
                        eprintln!(
                            "error: {}",
                            render_error(&crate::cli::CliError::UpdateFailed(
                                "asset not found".to_string()
                            ))
                        );
                        return Ok(());
                    }
                    let confirmed = if parsed.yes {
                        true
                    } else {
                        println!("Do you want to update? [y/N]");
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
                    };
                    if !confirmed {
                        println!("self-update: cancelled");
                        return Ok(());
                    }
                    let asset = plan.asset.as_ref().expect("asset exists");
                    match download_and_verify_asset(asset) {
                        Ok(path) => match replace_current_exe(&path, &plan.target_tag) {
                            Ok(backup) => {
                                println!("self-update: updated (backup: {})", backup.display());
                            }
                            Err(error) => {
                                eprintln!("error: {}", render_error(&crate::cli::CliError::UpdateFailed(error.to_string())));
                                return Ok(());
                            }
                        },
                        Err(error) => {
                            eprintln!("error: {}", render_error(&crate::cli::CliError::UpdateFailed(error.to_string())));
                            return Ok(());
                        }
                    }
                }
                Err(error) => {
                    eprintln!("error: {}", render_error(&error));
                    eprintln!("usage: ox self-update --tag <version>");
                    return Ok(());
                }
            }
            Ok(())
        }
        Command::SelfUpdateRollback { yes } => {
            let current_exe = std::env::current_exe()?;
            match crate::core::self_update::list_backups(&current_exe) {
                Ok(backups) => {
                    if backups.is_empty() {
                        eprintln!("error: {}", render_error(&crate::cli::CliError::UpdateFailed("no backups found".to_string())));
                        return Ok(());
                    }
                    println!("self-update: backups");
                    for (index, path) in backups.iter().enumerate() {
                        println!("  [{}] {}", index + 1, path.display());
                    }
                    let selection = if yes {
                        1
                    } else {
                        println!("Select backup number:");
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        input.trim().parse::<usize>().unwrap_or(0)
                    };
                    if selection == 0 || selection > backups.len() {
                        eprintln!("error: {}", render_error(&crate::cli::CliError::UpdateFailed("invalid selection".to_string())));
                        return Ok(());
                    }
                    let backup = &backups[selection - 1];
                    match crate::core::self_update::rollback_named(backup) {
                        Ok(path) => {
                            println!("self-update: rolled back from {}", path.display());
                        }
                        Err(error) => {
                            eprintln!("error: {}", render_error(&crate::cli::CliError::UpdateFailed(error.to_string())));
                            return Ok(());
                        }
                    }
                }
                Err(error) => {
                    eprintln!("error: {}", render_error(&crate::cli::CliError::UpdateFailed(error.to_string())));
                    return Ok(());
                }
            }
            Ok(())
        }
    }
}
