#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use chrono::NaiveDate;
use clap::Parser;
use eframe::egui::{self, Color32, RichText};
use spendcraft_core::*;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "spendcraft-app", about = "Native egui desktop host for SpendCraft")]
struct Args {
    #[arg(long)]
    smoke: bool,
    #[arg(long)]
    db: Option<PathBuf>,
}

fn resolve_db(path: Option<PathBuf>) -> PathBuf {
    if let Some(p) = path {
        return p;
    }
    if let Ok(p) = std::env::var("SPENDCRAFT_DB") {
        return PathBuf::from(p);
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let dir = PathBuf::from(local).join("spendcraft");
        let _ = std::fs::create_dir_all(&dir);
        return dir.join("spendcraft.db");
    }
    PathBuf::from("spendcraft.db")
}

fn run_smoke_checks() -> Result<(), Box<dyn std::error::Error>> {
    let store = SpendStore::open_in_memory()?;
    let claim = store.submit_claim(
        "SMOKE-001",
        "EMP-01",
        "Sandra Schipal",
        Some("AT000000000000000000"),
        "travel_per_diem",
        SpendStore::calculate_tagesdiaeten(12.0, false, false),
        0,
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap_or_default(),
        None,
        None,
        Some("Smoke Austrian per-diem check"),
    )?;
    if claim.amount_cents != 2640 {
        return Err("Austrian per diem calculation mismatch".into());
    }
    println!("smoke: ok (spendcraft-app)");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.smoke {
        return run_smoke_checks();
    }

    let db_path = resolve_db(args.db);
    let store = SpendStore::open(&db_path)?;
    let app = SpendApp::new(store);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 680.0])
            .with_title("SpendCraft — Expenses & Austrian Reisekosten (§26 EStG)"),
        ..Default::default()
    };

    eframe::run_native(
        "SpendCraft",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .map_err(|e| format!("Eframe runtime error: {e}").into())
}

struct SpendApp {
    store: SpendStore,
    claims: Vec<ExpenseClaim>,
    stats: SpendStats,
    selected_claim: Option<String>,
    calc_hours: f64,
    calc_km: f64,
    calc_result: Option<String>,
}

impl SpendApp {
    fn new(store: SpendStore) -> Self {
        let claims = store.list_claims(None).unwrap_or_default();
        let stats = store.stats().unwrap_or(SpendStats {
            total_claims: 0,
            total_amount_cents: 0,
            approved_amount_cents: 0,
            reimbursed_amount_cents: 0,
            pending_approval_count: 0,
        });
        Self {
            store,
            claims,
            stats,
            selected_claim: None,
            calc_hours: 8.0,
            calc_km: 150.0,
            calc_result: None,
        }
    }

    fn refresh(&mut self) {
        if let Ok(cs) = self.store.list_claims(None) {
            self.claims = cs;
        }
        if let Ok(st) = self.store.stats() {
            self.stats = st;
        }
    }
}

impl eframe::App for SpendApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("💳 SpendCraft").strong().color(Color32::from_rgb(243, 156, 18)));
            ui.separator();
            ui.label(RichText::new("Expense Management & Austrian Reisekosten (§26 EStG)").weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🔄 Refresh").clicked() {
                    self.refresh();
                }
            });
        });
        ui.add_space(4.0);

        // Live KPI HUD
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Total Volume: €{:.2}", self.stats.total_amount_cents as f64 / 100.0)).strong());
            ui.separator();
            ui.label(RichText::new(format!("Approved: €{:.2}", self.stats.approved_amount_cents as f64 / 100.0)).color(Color32::from_rgb(46, 204, 113)));
            ui.separator();
            ui.label(RichText::new(format!("Reimbursed: €{:.2}", self.stats.reimbursed_amount_cents as f64 / 100.0)).color(Color32::from_rgb(52, 152, 219)));
            ui.separator();
            ui.label(RichText::new(format!("Pending Review: {}", self.stats.pending_approval_count)).color(Color32::from_rgb(241, 196, 15)));
        });
        ui.separator();

        ui.horizontal(|ui| {
            // Left main table
            ui.vertical(|ui| {
                ui.heading("Claims Registry");
                egui::ScrollArea::vertical().max_width(620.0).show(ui, |ui| {
                    egui::Grid::new("spend_grid").striped(true).min_col_width(90.0).show(ui, |ui| {
                        ui.label(RichText::new("ID").strong());
                        ui.label(RichText::new("Employee").strong());
                        ui.label(RichText::new("Category").strong());
                        ui.label(RichText::new("Status").strong());
                        ui.label(RichText::new("Amount").strong());
                        ui.label(RichText::new("Action").strong());
                        ui.end_row();

                        let mut select_claim = None;
                        for c in &self.claims {
                            ui.label(&c.id);
                            ui.label(&c.employee_name);
                            ui.label(&c.category);
                            let status_color = match c.status {
                                ClaimStatus::Approved => Color32::from_rgb(46, 204, 113),
                                ClaimStatus::Reimbursed => Color32::from_rgb(52, 152, 219),
                                ClaimStatus::Rejected => Color32::from_rgb(231, 76, 60),
                                ClaimStatus::Submitted => Color32::from_rgb(241, 196, 15),
                            };
                            ui.label(RichText::new(c.status.as_str()).color(status_color));
                            ui.label(format!("€{:.2}", c.amount_cents as f64 / 100.0));
                            if ui.button("Review").clicked() {
                                select_claim = Some(c.id.clone());
                            }
                            ui.end_row();
                        }

                        if let Some(id) = select_claim {
                            self.selected_claim = Some(id);
                            self.refresh();
                        }
                    });
                });
            });

            ui.separator();

            // Right inspector & Austrian calculator
            ui.vertical(|ui| {
                if let Some(id) = &self.selected_claim.clone() {
                    ui.heading(format!("Claim: {id}"));
                    ui.add_space(4.0);

                    if let Ok(c) = self.store.get_claim(id) {
                        ui.label(format!("Employee: {} ({})", c.employee_name, c.employee_id));
                        ui.label(format!("Category: {}", c.category));
                        ui.label(format!("Status: {:?}", c.status));
                        ui.label(format!("Claim Date: {}", c.claim_date));
                        ui.separator();
                        ui.label(RichText::new(format!("Amount: €{:.2}", c.amount_cents as f64 / 100.0)).strong().size(18.0));
                        if let Some(n) = &c.notes {
                            ui.label(format!("Notes: {n}"));
                        }
                        ui.separator();

                        ui.horizontal(|ui| {
                            if ui.button("Approve").clicked() {
                                let _ = self.store.update_status(id, ClaimStatus::Approved);
                                self.refresh();
                            }
                            if ui.button("Reject").clicked() {
                                let _ = self.store.update_status(id, ClaimStatus::Rejected);
                                self.refresh();
                            }
                            if ui.button("Reimburse (Paid)").clicked() {
                                let _ = self.store.update_status(id, ClaimStatus::Reimbursed);
                                self.refresh();
                            }
                        });
                    }
                    ui.separator();
                }

                // Austrian §26 EStG Quick Calculator Dock
                ui.heading("🇦🇹 §26 EStG Reisekosten Calculator");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Travel Hours:");
                    ui.add(egui::DragValue::new(&mut self.calc_hours).range(0.0..=24.0).speed(0.5));
                });
                ui.horizontal(|ui| {
                    ui.label("Distance (km):");
                    ui.add(egui::DragValue::new(&mut self.calc_km).range(0.0..=2000.0).speed(5.0));
                });

                if ui.button("⚡ Calculate Statutory Allowances").clicked() {
                    let diaeten = SpendStore::calculate_tagesdiaeten(self.calc_hours, false, false);
                    let km_geld = SpendStore::calculate_kilometergeld(self.calc_km, "car");
                    let total = diaeten + km_geld;
                    self.calc_result = Some(format!(
                        "Tagesdiäten: €{:.2} | Kilometergeld: €{:.2} | Total: €{:.2}",
                        diaeten as f64 / 100.0,
                        km_geld as f64 / 100.0,
                        total as f64 / 100.0
                    ));
                }

                if let Some(res) = &self.calc_result {
                    ui.add_space(4.0);
                    ui.label(RichText::new(res).color(Color32::from_rgb(46, 204, 113)).strong());
                }
            });
        });
    }
}
