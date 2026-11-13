#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use chrono::{Local, NaiveDate};
use clap::Parser;
use craft_biz::bus::Bus;
use craft_biz::Money;
use craft_biz_ui::bookend::{self, Bar, Spec};
use craft_biz_ui::menu::Item;
use craft_biz_ui::shell::UiSettings;
use craft_biz_ui::theme::{self, Tone};
use craft_biz_ui::widgets;
use eframe::egui::{self, Color32, RichText};
use spendcraft_core::*;
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

fn resolve_bus() -> Option<Bus> {
    if let Ok(dir) = std::env::var("CRAFT_BUS_DIR") {
        return Some(Bus::new(PathBuf::from(dir)));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return Some(Bus::new(PathBuf::from(local).join("craft-biz-bus")));
    }
    None
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

    // 1. Austrian statutory Tagesdiäten check
    let diaeten = SpendStore::calculate_tagesdiaeten(12.0, false, false);
    if diaeten != 2640 {
        return Err("Austrian per diem calculation mismatch".into());
    }

    // 2. Fahrtenbuch Kilometergeld check with passengers
    let (rate, km_allowance) = SpendStore::calculate_trip_allowance(100.0, "car", 2);
    if rate != 52 || km_allowance != 5200 {
        return Err("Fahrtenbuch passenger surcharge mismatch".into());
    }

    // 3. Foreign per diem check (Germany)
    let de_rate = SpendStore::calculate_foreign_per_diem("DE", 12.0, false, false, false);
    if de_rate != 3530 {
        return Err("BMF foreign per diem rate mismatch".into());
    }

    // 4. Submit claim & store verification
    let claim = store.submit_claim_extended(
        "SMOKE-001",
        "EMP-01",
        "Sandra Schipal",
        Some("AT000000000000000000"),
        "travel_per_diem",
        2640,
        2640,
        0,
        "NONE",
        true,
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap_or_default(),
        None,
        None,
        Some("Smoke Austrian per-diem check"),
    )?;
    if claim.amount_cents != 2640 {
        return Err("Claim submission cents mismatch".into());
    }

    bookend::smoke("SpendCraft", env!("CARGO_PKG_VERSION"), "expenses")?;
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
    let app = SpendApp::new(store, db_path, args.dark);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([960.0, 600.0])
            .with_title("SpendCraft 💳 — Sovereign Expense & Reisekosten (§26 EStG)"),
        ..Default::default()
    };

    eframe::run_native(
        "SpendCraft",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
    .map_err(|e| format!("Eframe runtime error: {e}").into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Claims,
    Fahrtenbuch,
    Reisekosten,
    SepaBatches,
}

struct SpendApp {
    service: SpendService,
    db_path: PathBuf,
    settings: UiSettings,
    active_tab: ActiveTab,

    // Data cache
    claims: Vec<ExpenseClaim>,
    trips: Vec<Trip>,
    batches: Vec<ReimbursementBatch>,
    stats: SpendStats,

    // Claims View State
    selected_claim: Option<String>,
    filter_status: Option<ClaimStatus>,
    show_new_claim_modal: bool,
    new_claim_id: String,
    new_claim_emp_id: String,
    new_claim_emp_name: String,
    new_claim_iban: String,
    new_claim_category: String,
    new_claim_gross: String,
    new_claim_tax_key: String,
    new_claim_notes: String,

    // Fahrtenbuch View State
    new_trip_origin: String,
    new_trip_dest: String,
    new_trip_purpose: String,
    new_trip_vehicle: String,
    new_trip_km: f64,
    new_trip_passengers: i64,

    // Reisekosten Calc State
    calc_country: String,
    calc_hours: f64,
    calc_free_breakfast: bool,
    calc_free_lunch: bool,
    calc_free_dinner: bool,
    calc_km: f64,
    calc_result: Option<String>,
    uk_miles: f64,
    uk_already: f64,
    uk_passengers: i64,
    uk_vehicle: String,
    us_miles: f64,
    us_date: String,
    mileage_result: Option<String>,

    // SEPA Batch State
    payer_name: String,
    payer_iban: String,
    payer_bic: String,
    last_exported_xml: Option<String>,

    toast_msg: Option<(String, std::time::Instant)>,
    bar: Bar,
}

impl SpendApp {
    fn new(store: SpendStore, db_path: PathBuf, dark_cli: bool) -> Self {
        let service = SpendService::new(store, resolve_bus());
        let claims = service.store().list_claims(None).unwrap_or_default();
        let trips = service.store().list_trips(None).unwrap_or_default();
        let batches = service.store().list_batches().unwrap_or_default();
        let stats = service.store().stats().unwrap_or(SpendStats {
            total_claims: 0,
            total_amount_cents: 0,
            approved_amount_cents: 0,
            reimbursed_amount_cents: 0,
            pending_approval_count: 0,
            total_trips: 0,
            total_trip_distance_km: 0.0,
            total_mileage_allowance_cents: 0,
        });

        let mut settings = UiSettings::default();
        if dark_cli {
            settings.dark = true;
        }

        Self {
            service,
            db_path,
            settings,
            active_tab: ActiveTab::Claims,
            claims,
            trips,
            batches,
            stats,
            selected_claim: None,
            filter_status: None,
            show_new_claim_modal: false,
            new_claim_id: format!("EXP-{}", Local::now().format("%Y%m%d%H%M")),
            new_claim_emp_id: "EMP-01".into(),
            new_claim_emp_name: "Sandra Schipal".into(),
            new_claim_iban: "AT021200000123456789".into(),
            new_claim_category: "Travel & Subsistence".into(),
            new_claim_gross: "120,00".into(),
            new_claim_tax_key: "U20".into(),
            new_claim_notes: "".into(),
            new_trip_origin: "Wien (Büro)".into(),
            new_trip_dest: "Linz (Kunde)".into(),
            new_trip_purpose: "Projekt-Kickoff & System-Audit".into(),
            new_trip_vehicle: "car".into(),
            new_trip_km: 184.0,
            new_trip_passengers: 0,
            calc_country: "AT".into(),
            calc_hours: 8.5,
            calc_free_breakfast: false,
            calc_free_lunch: false,
            calc_free_dinner: false,
            calc_km: 120.0,
            calc_result: None,
            uk_miles: 100.0,
            uk_already: 0.0,
            uk_passengers: 0,
            uk_vehicle: "car".into(),
            us_miles: 100.0,
            us_date: "2026-07-15".into(),
            mileage_result: None,
            payer_name: "Fleet Sovereign Back-Office GmbH".into(),
            payer_iban: "AT021200000123456789".into(),
            payer_bic: "BAWAGATW".into(),
            last_exported_xml: None,
            toast_msg: None,
            bar: Bar::new(),
        }
    }

    fn paint_menu(&mut self, ui: &mut egui::Ui) {
        let docs = bookend::repo_docs(env!("CARGO_MANIFEST_DIR"));
        let spec = Spec {
            name: "SpendCraft",
            version: env!("CARGO_PKG_VERSION"),
            description: "Expense claims, mileage, and per diem.",
            repository: "https://github.com/sandraschi/spendcraft",
            docs: &docs,
            dark: self.settings.dark,
            zoom: self.settings.zoom,
            file: vec![
                Item::action("Drain inbox", "app.drain"),
                Item::action("Refresh", "app.refresh"),
            ],
            view: vec![
                Item::action("Claims", "view.claims").checked(self.active_tab == ActiveTab::Claims),
                Item::action("Fahrtenbuch", "view.trips")
                    .checked(self.active_tab == ActiveTab::Fahrtenbuch),
                Item::action("Per diem", "view.per_diem")
                    .checked(self.active_tab == ActiveTab::Reisekosten),
                Item::action("SEPA batches", "view.sepa")
                    .checked(self.active_tab == ActiveTab::SepaBatches),
            ],
            middle: Vec::new(),
        };
        let change = self.bar.show(ui, &spec);
        if let Some(dark) = change.dark {
            self.settings.dark = dark;
        }
        if let Some(zoom) = change.zoom {
            self.settings.zoom = zoom;
        }
        match change.action.as_deref() {
            Some("app.refresh") => self.refresh(),
            Some("app.drain") => self.drain_inbox(),
            Some("view.claims") => self.active_tab = ActiveTab::Claims,
            Some("view.trips") => self.active_tab = ActiveTab::Fahrtenbuch,
            Some("view.per_diem") => self.active_tab = ActiveTab::Reisekosten,
            Some("view.sepa") => self.active_tab = ActiveTab::SepaBatches,
            _ => {}
        }
    }

    fn drain_inbox(&mut self) {
        match self.service.drain_inbox() {
            Ok(s) => {
                self.refresh();
                self.toast(format!(
                    "Inbox: {} events, {} claims.",
                    s.processed,
                    s.created_claims.len()
                ));
            }
            Err(e) => self.toast(e.to_string()),
        }
    }

    fn toast(&mut self, msg: impl Into<String>) {
        self.toast_msg = Some((msg.into(), std::time::Instant::now()));
    }

    fn refresh(&mut self) {
        if let Ok(cs) = self.service.store().list_claims(self.filter_status) {
            self.claims = cs;
        }
        if let Ok(ts) = self.service.store().list_trips(None) {
            self.trips = ts;
        }
        if let Ok(bs) = self.service.store().list_batches() {
            self.batches = bs;
        }
        if let Ok(st) = self.service.store().stats() {
            self.stats = st;
        }
    }
}

impl eframe::App for SpendApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.paint_menu(ui);
        theme::apply(ui.ctx(), self.settings.dark);

        // Top navigation & branding header
        ui.horizontal(|ui| {
            ui.heading(
                RichText::new("💳 SpendCraft")
                    .strong()
                    .color(Color32::from_rgb(243, 156, 18)),
            );
            ui.separator();
            ui.label(RichText::new("Sovereign Austrian Expense & Reisekosten (§26 EStG)").weak());

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .button(if self.settings.dark {
                        "☀ Light"
                    } else {
                        "🌙 Dark"
                    })
                    .clicked()
                {
                    self.settings.dark = !self.settings.dark;
                }
                if ui.button("🔄 Refresh").clicked() {
                    self.refresh();
                    self.toast("Refreshed data store");
                }
            });
        });
        ui.add_space(6.0);

        // View Tabs
        ui.horizontal(|ui| {
            let mut tab_button = |ui: &mut egui::Ui, tab: ActiveTab, label: &str| {
                let is_active = self.active_tab == tab;
                let text = if is_active {
                    RichText::new(label).strong()
                } else {
                    RichText::new(label)
                };
                if ui.selectable_label(is_active, text).clicked() {
                    self.active_tab = tab;
                }
            };

            tab_button(ui, ActiveTab::Claims, "📥 Claims Inbox");
            tab_button(ui, ActiveTab::Fahrtenbuch, "🚗 Fahrtenbuch (Trips)");
            tab_button(ui, ActiveTab::Reisekosten, "🇦🇹 §26 EStG Engine");
            tab_button(ui, ActiveTab::SepaBatches, "💳 SEPA pain.001 Batches");
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(6.0);

        // Tab Content
        match self.active_tab {
            ActiveTab::Claims => self.render_claims_view(ui),
            ActiveTab::Fahrtenbuch => self.render_fahrtenbuch_view(ui),
            ActiveTab::Reisekosten => self.render_reisekosten_view(ui),
            ActiveTab::SepaBatches => self.render_sepa_view(ui),
        }

        ui.add_space(8.0);
        ui.separator();
        // Bottom status line
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("DB: {}", self.db_path.display()))
                    .weak()
                    .size(11.0),
            );
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "Claims: {} | Trips: {} | Batches: {}",
                    self.stats.total_claims,
                    self.stats.total_trips,
                    self.batches.len()
                ))
                .weak()
                .size(11.0),
            );

            if let Some((msg, time)) = &self.toast_msg {
                if time.elapsed().as_secs() < 4 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        widgets::pill(ui, msg, Tone::Ok);
                    });
                }
            }
        });
    }
}

impl SpendApp {
    fn render_claims_view(&mut self, ui: &mut egui::Ui) {
        // Executive KPI Tiles HUD
        ui.horizontal(|ui| {
            let total_fmt = Money::from_cents(self.stats.total_amount_cents).format_eur();
            let apprv_fmt = Money::from_cents(self.stats.approved_amount_cents).format_eur();
            let reimb_fmt = Money::from_cents(self.stats.reimbursed_amount_cents).format_eur();

            widgets::kpi(
                ui,
                "Total Claim Volume",
                &total_fmt,
                Some(("All time", Tone::Neutral)),
            );
            widgets::kpi(
                ui,
                "Approved Claims",
                &apprv_fmt,
                Some(("Awaiting payout", Tone::Ok)),
            );
            widgets::kpi(ui, "Reimbursed", &reimb_fmt, Some(("Settled", Tone::Info)));
            widgets::kpi(
                ui,
                "Pending Audit",
                &format!("{}", self.stats.pending_approval_count),
                Some(("Review needed", Tone::Warn)),
            );
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        // Action & Filter Bar
        ui.horizontal(|ui| {
            if ui.button("➕ Log New Claim").clicked() {
                self.show_new_claim_modal = true;
                self.new_claim_id = format!("EXP-{}", Local::now().format("%Y%m%d%H%M"));
            }
            ui.separator();
            ui.label("Filter Status:");

            let mut filter_btn = |ui: &mut egui::Ui, st: Option<ClaimStatus>, label: &str| {
                let is_sel = self.filter_status == st;
                if ui.selectable_label(is_sel, label).clicked() {
                    self.filter_status = st;
                    self.refresh();
                }
            };

            filter_btn(ui, None, "All");
            filter_btn(ui, Some(ClaimStatus::Submitted), "Submitted");
            filter_btn(ui, Some(ClaimStatus::Approved), "Approved");
            filter_btn(ui, Some(ClaimStatus::Reimbursed), "Reimbursed");
            filter_btn(ui, Some(ClaimStatus::Rejected), "Rejected");
        });
        ui.add_space(8.0);

        // Split view: Left table, Right inspector
        ui.columns(2, |cols| {
            // Left: Claims Table
            cols[0].vertical(|ui| {
                ui.heading("Claims Registry");
                ui.add_space(4.0);

                egui::ScrollArea::vertical().id_salt("claims_scroll").max_height(480.0).show(ui, |ui| {
                    egui::Grid::new("claims_grid").striped(true).min_col_width(80.0).show(ui, |ui| {
                        ui.label(RichText::new("ID").strong());
                        ui.label(RichText::new("Employee").strong());
                        ui.label(RichText::new("Category").strong());
                        ui.label(RichText::new("Tax Key").strong());
                        ui.label(RichText::new("Gross").strong());
                        ui.label(RichText::new("Status").strong());
                        ui.label(RichText::new("Action").strong());
                        ui.end_row();

                        let mut selected_id = None;
                        for c in &self.claims {
                            ui.label(&c.id);
                            ui.label(&c.employee_name);
                            ui.label(&c.category);
                            ui.label(&c.tax_key);
                            ui.label(Money::from_cents(c.amount_cents).format_eur());

                            let (tone, txt) = match c.status {
                                ClaimStatus::Approved => (Tone::Ok, "approved"),
                                ClaimStatus::Reimbursed => (Tone::Info, "reimbursed"),
                                ClaimStatus::Rejected => (Tone::Danger, "rejected"),
                                ClaimStatus::Submitted => (Tone::Warn, "submitted"),
                            };
                            widgets::pill(ui, txt, tone);

                            if ui.button("Review").clicked() {
                                selected_id = Some(c.id.clone());
                            }
                            ui.end_row();
                        }

                        if let Some(id) = selected_id {
                            self.selected_claim = Some(id);
                        }
                    });
                });
            });

            // Right: Claim Inspector
            cols[1].vertical(|ui| {
                if let Some(id) = self.selected_claim.clone() {
                    ui.heading(format!("Claim Details: {id}"));
                    ui.add_space(6.0);

                    if let Ok(c) = self.service.store().get_claim(&id) {
                        widgets::card(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Employee:").strong());
                                ui.label(format!("{} ({})", c.employee_name, c.employee_id));
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("IBAN:").strong());
                                ui.label(c.employee_iban.as_deref().unwrap_or("No IBAN specified"));
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Category:").strong());
                                ui.label(&c.category);
                            });
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Date:").strong());
                                ui.label(c.claim_date.to_string());
                            });
                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Gross Amount:").size(16.0).strong());
                                ui.label(RichText::new(Money::from_cents(c.amount_cents).format_eur()).size(16.0).color(Color32::from_rgb(46, 204, 113)).strong());
                            });
                            ui.horizontal(|ui| {
                                ui.label("Net:");
                                ui.label(Money::from_cents(c.net_cents).format_eur());
                                ui.separator();
                                ui.label("VAT Tax:");
                                ui.label(format!("{} ({})", Money::from_cents(c.tax_cents).format_eur(), c.tax_key));
                            });

                            if c.is_kleinbetragsrechnung {
                                widgets::pill(ui, "Kleinbetragsrechnung (§11 Abs 6 UStG < €400)", Tone::Info);
                            }

                            if let Some(n) = &c.notes {
                                ui.add_space(4.0);
                                ui.label(RichText::new("Notes / Business Justification:").strong());
                                ui.label(n);
                            }

                            ui.add_space(8.0);
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui.button("✓ Approve Claim").clicked() {
                                    let _ = self.service.store().update_status(&id, ClaimStatus::Approved);
                                    self.refresh();
                                    self.toast(format!("Claim {id} approved"));
                                }
                                if ui.button("✗ Reject").clicked() {
                                    let _ = self.service.store().update_status(&id, ClaimStatus::Rejected);
                                    self.refresh();
                                    self.toast(format!("Claim {id} rejected"));
                                }
                                if ui.button("💳 Mark Reimbursed").clicked() {
                                    let _ = self.service.store().update_status(&id, ClaimStatus::Reimbursed);
                                    self.refresh();
                                    self.toast(format!("Claim {id} marked reimbursed"));
                                }
                            });
                        });
                    }
                } else {
                    widgets::empty_state(ui, "No Claim Selected", "Select any claim from the table on the left to audit receipt, VAT tax split, and approve payout.");
                }
            });
        });

        // New Claim Modal
        if self.show_new_claim_modal {
            egui::Window::new("Submit New Expense Claim")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Claim ID:");
                        ui.text_edit_singleline(&mut self.new_claim_id);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Employee ID:");
                        ui.text_edit_singleline(&mut self.new_claim_emp_id);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Employee Name:");
                        ui.text_edit_singleline(&mut self.new_claim_emp_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("IBAN:");
                        ui.text_edit_singleline(&mut self.new_claim_iban);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Category:");
                        ui.text_edit_singleline(&mut self.new_claim_category);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Gross Amount (EUR):");
                        ui.text_edit_singleline(&mut self.new_claim_gross);
                    });
                    ui.horizontal(|ui| {
                        ui.label("VAT Tax Key:");
                        egui::ComboBox::from_id_salt("tax_key_select")
                            .selected_text(&self.new_claim_tax_key)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.new_claim_tax_key,
                                    "U20".into(),
                                    "U20 (20% Standard)",
                                );
                                ui.selectable_value(
                                    &mut self.new_claim_tax_key,
                                    "U10".into(),
                                    "U10 (10% Food/Books)",
                                );
                                ui.selectable_value(
                                    &mut self.new_claim_tax_key,
                                    "U13".into(),
                                    "U13 (13% Travel)",
                                );
                                ui.selectable_value(
                                    &mut self.new_claim_tax_key,
                                    "NONE".into(),
                                    "NONE (0% / Exempt)",
                                );
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Business Notes:");
                        ui.text_edit_singleline(&mut self.new_claim_notes);
                    });

                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save Claim").clicked() {
                            let gross_res = Money::parse_de(&self.new_claim_gross);
                            if let Ok(gross) = gross_res {
                                let rate_pct = match self.new_claim_tax_key.as_str() {
                                    "U20" => 20,
                                    "U10" => 10,
                                    "U13" => 13,
                                    _ => 0,
                                };
                                let (net, tax) = Money::split_gross(gross, rate_pct)
                                    .unwrap_or((gross, Money::ZERO));
                                let is_kb = gross.cents() <= 40000;

                                let _ = self.service.store().submit_claim_extended(
                                    &self.new_claim_id,
                                    &self.new_claim_emp_id,
                                    &self.new_claim_emp_name,
                                    Some(&self.new_claim_iban),
                                    &self.new_claim_category,
                                    gross.cents(),
                                    net.cents(),
                                    tax.cents(),
                                    &self.new_claim_tax_key,
                                    is_kb,
                                    Local::now().date_naive(),
                                    None,
                                    None,
                                    Some(&self.new_claim_notes),
                                );
                                self.show_new_claim_modal = false;
                                self.refresh();
                                self.toast(format!("Claim {} submitted", self.new_claim_id));
                            }
                        }
                        if ui.button("Cancel").clicked() {
                            self.show_new_claim_modal = false;
                        }
                    });
                });
        }
    }

    fn render_fahrtenbuch_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let km_tile = format!("{:.1} km", self.stats.total_trip_distance_km);
            let money_tile =
                Money::from_cents(self.stats.total_mileage_allowance_cents).format_eur();

            widgets::kpi(
                ui,
                "Logged Business Trips",
                &format!("{}", self.stats.total_trips),
                Some(("Fahrtenbuch", Tone::Neutral)),
            );
            widgets::kpi(
                ui,
                "Total Distance",
                &km_tile,
                Some(("Auditable", Tone::Info)),
            );
            widgets::kpi(
                ui,
                "Kilometergeld",
                &money_tile,
                Some(("§26 Z4 EStG", Tone::Ok)),
            );
        });
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.columns(2, |cols| {
            // Left: Log Trip Form
            cols[0].vertical(|ui| {
                ui.heading("Log Business Trip (§26 Z4 EStG)");
                ui.label(
                    RichText::new("Complies with statutory Austrian Fahrtenbuch requirements.")
                        .weak(),
                );
                ui.add_space(6.0);

                widgets::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Origin:");
                        ui.text_edit_singleline(&mut self.new_trip_origin);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Destination:");
                        ui.text_edit_singleline(&mut self.new_trip_dest);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Business Purpose:");
                        ui.text_edit_singleline(&mut self.new_trip_purpose);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Vehicle:");
                        egui::ComboBox::from_id_salt("vehicle_select")
                            .selected_text(match self.new_trip_vehicle.as_str() {
                                "car" => "Car (PKW standard: €0.42/km)",
                                "car_2026" => "Car 2026 Reform (€0.50/km)",
                                "bicycle" => "Bicycle / E-Bike (€0.40/km)",
                                "motorcycle" => "Motorcycle (€0.24/km)",
                                _ => "Car (Standard)",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.new_trip_vehicle,
                                    "car".into(),
                                    "Car (PKW: €0.42/km)",
                                );
                                ui.selectable_value(
                                    &mut self.new_trip_vehicle,
                                    "car_2026".into(),
                                    "Car 2026 Reform (€0.50/km)",
                                );
                                ui.selectable_value(
                                    &mut self.new_trip_vehicle,
                                    "bicycle".into(),
                                    "Bicycle (€0.40/km)",
                                );
                                ui.selectable_value(
                                    &mut self.new_trip_vehicle,
                                    "motorcycle".into(),
                                    "Motorcycle (€0.24/km)",
                                );
                            });
                    });
                    ui.horizontal(|ui| {
                        ui.label("Distance (km):");
                        ui.add(
                            egui::DragValue::new(&mut self.new_trip_km)
                                .range(1.0..=2500.0)
                                .speed(5.0),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label("Mitfahrer (Passengers):");
                        ui.add(egui::DragValue::new(&mut self.new_trip_passengers).range(0..=8));
                        ui.label(
                            RichText::new("(+€0.05/km statutory surcharge per passenger)").weak(),
                        );
                    });

                    let (rate, total) = SpendStore::calculate_trip_allowance(
                        self.new_trip_km,
                        &self.new_trip_vehicle,
                        self.new_trip_passengers,
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "Calculated Rate: €{:.2}/km | Total Allowance: {}",
                            rate as f64 / 100.0,
                            Money::from_cents(total).format_eur()
                        ))
                        .color(Color32::from_rgb(46, 204, 113))
                        .strong(),
                    );

                    ui.add_space(8.0);
                    if ui.button("💾 Record Trip in Fahrtenbuch").clicked() {
                        let trip_id = format!("TRIP-{}", Local::now().format("%Y%m%d%H%M%S"));
                        let _ = self.service.store().record_trip(
                            &trip_id,
                            "EMP-01",
                            "Sandra Schipal",
                            Local::now().date_naive(),
                            Some("08:30"),
                            Some("17:45"),
                            &self.new_trip_origin,
                            &self.new_trip_dest,
                            &self.new_trip_purpose,
                            &self.new_trip_vehicle,
                            None,
                            None,
                            self.new_trip_km,
                            self.new_trip_passengers,
                        );
                        self.refresh();
                        self.toast(format!("Trip {trip_id} recorded in Fahrtenbuch"));
                    }
                });
            });

            // Right: Trips Log Table
            cols[1].vertical(|ui| {
                ui.heading("Fahrtenbuch Registry");
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("trips_scroll")
                    .max_height(480.0)
                    .show(ui, |ui| {
                        egui::Grid::new("trips_grid")
                            .striped(true)
                            .min_col_width(75.0)
                            .show(ui, |ui| {
                                ui.label(RichText::new("Date").strong());
                                ui.label(RichText::new("Route").strong());
                                ui.label(RichText::new("Vehicle").strong());
                                ui.label(RichText::new("Distance").strong());
                                ui.label(RichText::new("Allowance").strong());
                                ui.label(RichText::new("Action").strong());
                                ui.end_row();

                                let mut delete_trip_id = None;
                                for t in &self.trips {
                                    ui.label(t.trip_date.to_string());
                                    ui.label(format!("{} → {}", t.origin, t.destination));
                                    ui.label(&t.vehicle);
                                    ui.label(format!("{:.1} km", t.distance_km));
                                    ui.label(Money::from_cents(t.allowance_cents).format_eur());

                                    if ui.button("Delete").clicked() {
                                        delete_trip_id = Some(t.id.clone());
                                    }
                                    ui.end_row();
                                }

                                if let Some(id) = delete_trip_id {
                                    let _ = self.service.store().delete_trip(&id);
                                    self.refresh();
                                    self.toast(format!("Trip {id} deleted"));
                                }
                            });
                    });
            });
        });
    }

    fn render_reisekosten_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("🇦🇹 Austrian Statutory Reisekosten Engine (§26 EStG)");
        ui.label(RichText::new("Austrian foreign-travel day rates. A GB or US row here is not HMRC subsistence and not GSA per diem. UK mileage is uk-amap-2026 and US mileage is us-irs-mileage-2026, further down.").weak());
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                widgets::card(ui, |ui| {
                    ui.heading("Travel Parameters");
                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label("Destination Country:");
                        egui::ComboBox::from_id_salt("country_select")
                            .selected_text(match self.calc_country.as_str() {
                                "AT" => "Österreich (Inland: €26.40/day)",
                                "DE" => "Deutschland (€35.30/day)",
                                "CH" => "Schweiz (€42.40/day)",
                                "IT" => "Italien (€36.00/day)",
                                "GB" => "Großbritannien (€40.00/day)",
                                "CZ" => "Tschechien (€32.00/day)",
                                "HU" => "Ungarn (€29.00/day)",
                                "US" => "USA (€52.00/day)",
                                _ => "Österreich",
                            })
                            .show_ui(ui, |ui| {
                                for r in BMF_PER_DIEM_RATES {
                                    ui.selectable_value(&mut self.calc_country, r.country_code.to_string(), format!("{} (€{:.2}/day)", r.country_name, r.daily_cents as f64 / 100.0));
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Travel Hours:");
                        ui.add(egui::DragValue::new(&mut self.calc_hours).range(0.0..=24.0).speed(0.5));
                        ui.label(RichText::new("(>3h threshold, >=11h full day rate)").weak());
                    });

                    ui.horizontal(|ui| {
                        ui.label("Provided Meals Deductions:");
                    });
                    ui.checkbox(&mut self.calc_free_breakfast, "Breakfast provided by hotel (-15%)");
                    ui.checkbox(&mut self.calc_free_lunch, "Lunch provided by employer/host (-50%)");
                    ui.checkbox(&mut self.calc_free_dinner, "Dinner provided by employer/host (-50%)");

                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Additional Travel Distance (km):");
                        ui.add(egui::DragValue::new(&mut self.calc_km).range(0.0..=2000.0).speed(10.0));
                    });

                    ui.add_space(8.0);
                    if ui.button("⚡ Calculate Statutory Allowances").clicked() {
                        let diaeten = SpendStore::calculate_foreign_per_diem(
                            &self.calc_country,
                            self.calc_hours,
                            self.calc_free_breakfast,
                            self.calc_free_lunch,
                            self.calc_free_dinner,
                        );
                        let km_geld = SpendStore::calculate_kilometergeld(self.calc_km, "car");
                        let total = diaeten + km_geld;

                        self.calc_result = Some(format!(
                            "Country: {} | Tagesdiäten: {} | Kilometergeld: {} | Total Payout: {}",
                            self.calc_country,
                            Money::from_cents(diaeten).format_eur(),
                            Money::from_cents(km_geld).format_eur(),
                            Money::from_cents(total).format_eur()
                        ));
                    }
                });
            });

            cols[1].vertical(|ui| {
                let summary_opt = self.calc_result.clone();
                if let Some(res) = summary_opt {
                    widgets::card(ui, |ui| {
                        ui.heading("Calculation Summary");
                        ui.add_space(6.0);
                        ui.label(RichText::new(&res).color(Color32::from_rgb(46, 204, 113)).strong().size(15.0));
                        ui.add_space(8.0);

                        if ui.button("📥 Generate Expense Claim from Calculation").clicked() {
                            let diaeten = SpendStore::calculate_foreign_per_diem(
                                &self.calc_country,
                                self.calc_hours,
                                self.calc_free_breakfast,
                                self.calc_free_lunch,
                                self.calc_free_dinner,
                            );
                            let km_geld = SpendStore::calculate_kilometergeld(self.calc_km, "car");
                            let total = diaeten + km_geld;
                            let claim_id = format!("EXP-CALC-{}", Local::now().format("%Y%m%d%H%M%S"));

                            let _ = self.service.store().submit_claim_extended(
                                &claim_id,
                                "EMP-01",
                                "Sandra Schipal",
                                Some("AT021200000123456789"),
                                "travel_per_diem",
                                total,
                                total,
                                0,
                                "NONE",
                                true,
                                Local::now().date_naive(),
                                None,
                                None,
                                Some(&format!("Austrian §26 EStG calculation: {res}")),
                            );
                            self.refresh();
                            self.toast(format!("Claim {claim_id} created"));
                        }
                    });
                } else {
                    widgets::empty_state(ui, "No Calculation Yet", "Enter your travel hours, destination country, and meals on the left to see the official statutory allowance breakdown.");
                }
            });
        });
        ui.add_space(8.0);
        ui.separator();
        ui.heading("UK AMAP 2026 and US IRS business mileage 2026");
        ui.label("These amounts stay in pence or US cents. They are not added to an Austrian euro claim.");
        ui.horizontal(|ui| {
            ui.label("UK miles");
            ui.add(
                egui::DragValue::new(&mut self.uk_miles)
                    .range(0.0..=100_000.0)
                    .speed(1.0),
            );
            ui.label("already this tax year");
            ui.add(
                egui::DragValue::new(&mut self.uk_already)
                    .range(0.0..=100_000.0)
                    .speed(10.0),
            );
            ui.label("passengers");
            ui.add(egui::DragValue::new(&mut self.uk_passengers).range(0..=8));
            ui.label("vehicle");
            ui.text_edit_singleline(&mut self.uk_vehicle);
        });
        ui.horizontal(|ui| {
            ui.label("US miles");
            ui.add(
                egui::DragValue::new(&mut self.us_miles)
                    .range(0.0..=100_000.0)
                    .speed(1.0),
            );
            ui.label("trip date");
            ui.text_edit_singleline(&mut self.us_date);
            if ui.button("Calculate UK and US mileage").clicked() {
                self.mileage_result = Some(self.foreign_mileage());
            }
        });
        if let Some(text) = &self.mileage_result {
            ui.label(text);
        }
    }

    fn foreign_mileage(&self) -> String {
        let uk = match SpendStore::calculate_uk_amap(
            self.uk_miles,
            &self.uk_vehicle,
            self.uk_passengers,
            self.uk_already,
        ) {
            Ok(r) => format!(
                "{}: {} pence for {} miles. {}",
                r.ruleset, r.minor_units, self.uk_miles, UK_AMAP_AS_OF
            ),
            Err(e) => format!("UK: {e}"),
        };
        let us = match NaiveDate::parse_from_str(self.us_date.trim(), "%Y-%m-%d") {
            Ok(on) => match SpendStore::calculate_us_business_mileage(self.us_miles, on) {
                Ok(r) => format!(
                    "{}: {} cents for {} miles on {}. {}",
                    r.ruleset, r.minor_units, self.us_miles, on, US_IRS_MILEAGE_AS_OF
                ),
                Err(e) => format!("US: {e}"),
            },
            Err(_) => "US: trip date is YYYY-MM-DD.".into(),
        };
        format!("{uk}\n{us}")
    }

    fn render_sepa_view(&mut self, ui: &mut egui::Ui) {
        ui.heading("💳 ISO 20022 SEPA Credit Transfer Generator (`pain.001.001.03`)");
        ui.label(RichText::new("Generates bank-compliant XML batch payment orders for George (Erste Bank), ELBA (Raiffeisen), or Bank Austria.").weak());
        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        let approved_claims: Vec<ExpenseClaim> = self
            .claims
            .iter()
            .filter(|c| c.status == ClaimStatus::Approved)
            .cloned()
            .collect();
        let total_approved_cents: i64 = approved_claims.iter().map(|c| c.amount_cents).sum();

        ui.horizontal(|ui| {
            widgets::kpi(
                ui,
                "Pending Batched Payout",
                &Money::from_cents(total_approved_cents).format_eur(),
                Some((&format!("{} claims", approved_claims.len()), Tone::Warn)),
            );
            widgets::kpi(
                ui,
                "Exported Batches",
                &format!("{}", self.batches.len()),
                Some(("XML orders", Tone::Info)),
            );
        });
        ui.add_space(8.0);

        ui.columns(2, |cols| {
            cols[0].vertical(|ui| {
                ui.heading("Payer Account Configuration");
                widgets::card(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Payer Name:");
                        ui.text_edit_singleline(&mut self.payer_name);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Payer IBAN:");
                        ui.text_edit_singleline(&mut self.payer_iban);
                    });
                    ui.horizontal(|ui| {
                        ui.label("Payer BIC:");
                        ui.text_edit_singleline(&mut self.payer_bic);
                    });

                    ui.add_space(8.0);
                    if approved_claims.is_empty() {
                        ui.label(RichText::new("No claims currently in 'Approved' status. Approve claims in Claims Inbox first.").weak());
                    } else if ui.button(format!("Generate SEPA pain.001 XML ({} claims, {})", approved_claims.len(), Money::from_cents(total_approved_cents).format_eur())).clicked() {
                        let ids: Vec<String> = approved_claims.iter().map(|c| c.id.clone()).collect();
                        let name = self.payer_name.clone();
                        let iban = self.payer_iban.clone();
                        let bic = self.payer_bic.clone();
                        match self.service.batch_reimburse(&ids, &name, &iban, &bic) {
                            Ok(batch) => {
                                self.last_exported_xml = Some(batch.xml_payload);
                                self.refresh();
                                self.toast(format!(
                                    "{} reimbursed {} claims and notified buchcraft and billcraft.",
                                    batch.batch_id, batch.count
                                ));
                            }
                            Err(e) => self.toast(format!("Error: {e}")),
                        }
                    }
                });
            });

            cols[1].vertical(|ui| {
                ui.heading("SEPA XML Preview");
                let xml_opt = self.last_exported_xml.clone();
                if let Some(xml) = xml_opt {
                    if ui.button("📋 Copy XML to Clipboard").clicked() {
                        ui.ctx().copy_text(xml.clone());
                        self.toast("XML copied to clipboard");
                    }
                    egui::ScrollArea::vertical().id_salt("xml_preview").max_height(350.0).show(ui, |ui| {
                        let mut text_buf = xml;
                        ui.text_edit_multiline(&mut text_buf);
                    });
                } else if let Some(b) = self.batches.first().cloned() {
                    ui.label(format!("Last Exported Batch: {} ({} claims, {})", b.id, b.claim_count, Money::from_cents(b.total_amount_cents).format_eur()));
                    egui::ScrollArea::vertical().id_salt("batch_xml").max_height(350.0).show(ui, |ui| {
                        let mut text_buf = b.xml_payload;
                        ui.text_edit_multiline(&mut text_buf);
                    });
                } else {
                    widgets::empty_state(ui, "No SEPA Batch Generated Yet", "Approve expense claims and click 'Generate SEPA pain.001 XML' to create an ISO 20022 payment order.");
                }
            });
        });
    }
}
