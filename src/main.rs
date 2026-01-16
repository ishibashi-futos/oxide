mod app;
mod core;
mod error;
mod opener;
mod ui;

use crate::{app::App, error::AppResult, opener::PlatformOpener};

fn main() -> AppResult<()> {
    let current_dir = std::env::current_dir()?;
    let app = App::load(current_dir)?;
    let opener = PlatformOpener;
    ui::run(app, &opener)
}
