#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use chrono::Local;
use clap::{Parser, Subcommand};
use craft_biz::bus::Bus;
use serde_json::json;
use spendcraft_core::*;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "spendcraft", author, version, about = "Local-first expense & Austrian Reisekosten (§26 EStG) engine. CLI + MCP.")]
struct Cli {
    #[arg(long)]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Status,
    Submit {
        id: String,
        employee_id: String,
        employee_name: String,
        category: String,
        amount_cents: i64,
        #[arg(long, default_value = "0")]
        tax_cents: i64,
        #[arg(long)]
        iban: Option<String>,
        #[arg(long)]
        notes: Option<String>,
    },
    Show {
        id: String,
    },
    List {
        #[arg(long)]
        status: Option<String>,
    },
    Approve {
        id: String,
    },
    Reject {
        id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    CalcDiaeten {
        hours: f64,
        #[arg(long)]
        free_lunch: bool,
        #[arg(long)]
        free_dinner: bool,
    },
    CalcKm {
        km: f64,
        #[arg(long, default_value = "car")]
        vehicle: String,
    },
    ExportSepa {
        claim_ids: Vec<String>,
        #[arg(long, default_value = "Fleet Organization")]
        payer_name: String,
        #[arg(long, default_value = "AT021200000123456789")]
        payer_iban: String,
        #[arg(long, default_value = "BAWAGATW")]
        payer_bic: String,
    },
    Stats,
    InboxDrain,
    Tools,
    Mcp,
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

fn resolve_bus() -> Option<Bus> {
    if let Ok(dir) = std::env::var("CRAFT_BUS_DIR") {
        return Some(Bus::new(PathBuf::from(dir)));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return Some(Bus::new(PathBuf::from(local).join("craft-biz-bus")));
    }
    None
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let db_path = resolve_db(cli.db);
    let store = SpendStore::open(&db_path)?;
    let bus = resolve_bus();
    let service = SpendService::new(store, bus);

    let Some(cmd) = cli.command else {
        println!("SpendCraft — Local-first expense & Austrian Reisekosten engine. Use --help for subcommands.");
        return Ok(());
    };

    match cmd {
        Commands::Status => {
            let s = service.store().stats()?;
            println!("Claims: {}", s.total_claims);
            println!("Total Volume: €{:.2}", s.total_amount_cents as f64 / 100.0);
            println!("Approved: €{:.2}", s.approved_amount_cents as f64 / 100.0);
            println!("Reimbursed: €{:.2}", s.reimbursed_amount_cents as f64 / 100.0);
            println!("Pending Approval: {}", s.pending_approval_count);
        }
        Commands::Submit { id, employee_id, employee_name, category, amount_cents, tax_cents, iban, notes } => {
            let today = Local::now().date_naive();
            let c = service.store().submit_claim(
                &id,
                &employee_id,
                &employee_name,
                iban.as_deref(),
                &category,
                amount_cents,
                tax_cents,
                today,
                None,
                None,
                notes.as_deref(),
            )?;
            println!("Submitted expense claim {} for {} (€{:.2})", c.id, c.employee_name, c.amount_cents as f64 / 100.0);
        }
        Commands::Show { id } => {
            let c = service.store().get_claim(&id)?;
            println!("Claim: {}", c.id);
            println!("Employee: {} ({})", c.employee_name, c.employee_id);
            println!("Category: {}", c.category);
            println!("Amount: €{:.2} (Tax: €{:.2})", c.amount_cents as f64 / 100.0, c.tax_cents as f64 / 100.0);
            println!("Status: {:?}", c.status);
            println!("Date: {}", c.claim_date);
            if let Some(n) = c.notes {
                println!("Notes: {}", n);
            }
        }
        Commands::List { status } => {
            let filter = status.as_deref().and_then(ClaimStatus::from_str_opt);
            let claims = service.store().list_claims(filter)?;
            for c in claims {
                println!("{:14} {:16} {:16} {:10} €{:.2}", c.id, c.employee_name, c.category, c.status.as_str(), c.amount_cents as f64 / 100.0);
            }
        }
        Commands::Approve { id } => {
            let c = service.approve_claim(&id)?;
            println!("Approved claim {} (€{:.2})", c.id, c.amount_cents as f64 / 100.0);
        }
        Commands::Reject { id, reason } => {
            let c = service.reject_claim(&id, reason.as_deref())?;
            println!("Rejected claim {}", c.id);
        }
        Commands::CalcDiaeten { hours, free_lunch, free_dinner } => {
            let cents = SpendStore::calculate_tagesdiaeten(hours, free_lunch, free_dinner);
            println!("Austrian §26 EStG Tagesdiäten for {:.1}h (Lunch: {}, Dinner: {}): €{:.2}", hours, free_lunch, free_dinner, cents as f64 / 100.0);
        }
        Commands::CalcKm { km, vehicle } => {
            let cents = SpendStore::calculate_kilometergeld(km, &vehicle);
            println!("Austrian §26 EStG Kilometergeld for {:.1}km ({}): €{:.2}", km, vehicle, cents as f64 / 100.0);
        }
        Commands::ExportSepa { claim_ids, payer_name, payer_iban, payer_bic } => {
            let xml = service.store().export_sepa_reimbursement(&claim_ids, &payer_name, &payer_iban, &payer_bic)?;
            println!("{xml}");
        }
        Commands::Stats => {
            let s = service.store().stats()?;
            println!("{}", serde_json::to_string_pretty(&s)?);
        }
        Commands::InboxDrain => {
            let res = service.drain_inbox()?;
            println!("Drained {} travel events from inbox. Created: {:?}", res.processed, res.created_claims);
        }
        Commands::Tools => {
            print_tools_catalog();
        }
        Commands::Mcp => {
            run_mcp_server(service)?;
        }
    }

    Ok(())
}

fn print_tools_catalog() {
    let catalog = json!([
        { "name": "spend_stats", "description": "Get expense claim totals and approval queue" },
        { "name": "expense_submit", "description": "Submit a new expense claim" },
        { "name": "expense_approve", "description": "Approve claim and emit event to billcraft" },
        { "name": "expense_reject", "description": "Reject claim with reason" },
        { "name": "expense_get", "description": "Get detailed claim information" },
        { "name": "expense_list", "description": "List claims filtered by status" },
        { "name": "calc_tagesdiaeten", "description": "Calculate Austrian §26 EStG statutory per diem" },
        { "name": "calc_kilometergeld", "description": "Calculate Austrian §26 EStG statutory mileage" },
        { "name": "export_sepa_reimbursement", "description": "Generate ISO 20022 pain.001 credit transfer batch" },
        { "name": "inbox_drain", "description": "Drain travel events from teamcraft to auto-create claims" }
    ]);
    println!("{}", serde_json::to_string_pretty(&catalog).unwrap_or_default());
}

fn run_mcp_server(service: SpendService) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let Ok(req) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };

        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let id = req.get("id").cloned();

        match method {
            "initialize" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "serverInfo": { "name": "spendcraft-mcp", "version": "0.1.0" },
                        "capabilities": { "tools": {} }
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }
            "tools/list" => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "tools": [
                            { "name": "spend_stats", "description": "Get spend totals", "inputSchema": { "type": "object" } },
                            { "name": "expense_list", "description": "List expense claims", "inputSchema": { "type": "object" } },
                            { "name": "calc_tagesdiaeten", "description": "Calculate Austrian §26 EStG per diem", "inputSchema": { "type": "object", "properties": { "hours": { "type": "number" } }, "required": ["hours"] } }
                        ]
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }
            "tools/call" => {
                let tool_name = req.get("params").and_then(|p| p.get("name")).and_then(|n| n.as_str()).unwrap_or("");
                let result_json = match tool_name {
                    "spend_stats" => {
                        let st = service.store().stats().map_err(|e| e.to_string())?;
                        json!(st)
                    }
                    "expense_list" => {
                        let claims = service.store().list_claims(None).map_err(|e| e.to_string())?;
                        json!(claims)
                    }
                    _ => json!({ "status": "unknown tool" }),
                };
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result_json)? }]
                    }
                });
                writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                stdout.flush()?;
            }
            _ => {
                if let Some(id) = id {
                    let resp = json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": "Method not found" } });
                    writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
                    stdout.flush()?;
                }
            }
        }
    }
    Ok(())
}
