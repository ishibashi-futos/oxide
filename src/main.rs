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
    Command, parse_args, parse_self_update_args, render_error, self_update_decision_line,
    self_update_latest_decision_line, self_update_intro, usage,
};
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
                    if let Some(tag) = parsed.tag.as_ref() {
                        match self_update_decision_line(&env, env!("CARGO_PKG_VERSION"), &parsed) {
                            Ok(Some(line)) => println!("{line}"),
                            Ok(None) => {}
                            Err(error) => {
                                eprintln!("error: {}", render_error(&error));
                                return Ok(());
                            }
                        }
                        println!("self-update: target tag {tag}");
                    } else {
                        match self_update_latest_decision_line(
                            &env,
                            env!("CARGO_PKG_VERSION"),
                            "ishibashi-futos/oxide",
                            &parsed,
                        ) {
                            Ok(Some(line)) => println!("{line}"),
                            Ok(None) => {}
                            Err(error) => {
                                eprintln!("error: {}", render_error(&error));
                                return Ok(());
                            }
                        }
                    }
                }
                Err(error) => {
                    eprintln!("error: {}", render_error(&error));
                    eprintln!("usage: ox self-update --tag <version>");
                    return Ok(());
                }
            }
            println!("self-update: not implemented yet");
            Ok(())
        }
        Command::SelfUpdateRollback => {
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
                    println!("Select backup number:");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    let selection = input.trim().parse::<usize>().unwrap_or(0);
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
