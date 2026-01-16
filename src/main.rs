mod app;
mod core;
mod error;
mod ui;

use crate::{app::App, error::AppResult};

fn main() -> AppResult<()> {
    let current_dir = std::env::current_dir()?;
    let entries = core::list_entries(&current_dir)?;
    let cursor = if entries.is_empty() { None } else { Some(0) };
    let app = App::new(current_dir, entries, cursor);
    ui::run(app)
}
