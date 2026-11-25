#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use clap::Parser;
use eframe::egui;
use spendcraft_app::{resolve_db, run_smoke_checks, SpendApp};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "spendcraft-app",
    about = "Sovereign Austrian Expense & Reisekosten (§26 EStG) Desktop App"
)]
struct Args {
    #[arg(long)]
    smoke: bool,
    #[arg(long)]
    db: Option<PathBuf>,
    #[arg(long)]
    dark: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.smoke {
        return run_smoke_checks();
    }

    let db_path = resolve_db(args.db);
    let app = SpendApp::open(db_path, args.dark)?;

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("SpendCraft - Sovereign Expense & Reisekosten (§26 EStG)"),
        ..Default::default()
    };

    eframe::run_native(
        "SpendCraft",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .map_err(|e| format!("Eframe runtime error: {e}").into())
}
