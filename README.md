# SpendCraft 💳

**Local-first sovereign expense management, Austrian statutory travel allowances (§26 EStG), and SEPA reimbursement engine.**

Part of the Sovereign Back-Office Suite replacing SaaS spend with zero-cloud-dependency, instant, offline-capable Rust native applications.

## Features

- **Austrian Statutory Travel Allowance Engine (§26 EStG)**:
  - **Kilometergeld**: Official Austrian rates (42c/km car, 40c/km bicycle) with detailed logbook tracking.
  - **Tagesdiäten**: €26.40 full-day domestic rate, pro-rated €2.20/hour for trips > 3 hours, statutory deductions for provided lunch (€13.20) and dinner (€13.20).
- **ISO 20022 SEPA Credit Transfer (`pain.001.001.03`)**: Generates compliant XML reimbursement batch payment orders directly importable into George (Erste Bank), ELBA (Raiffeisen), or Bank Austria.
- **Event Bus Integration**: Consumes `travel.logged` from `teamcraft`, emits `expense.approved` and `reimbursement.batched` to `billcraft\inbox`.
- **Tri-Modal Interface**:
  - **L1 CLI**: Quick claim logging and command-line approval.
  - **L2 MCP**: 8 FastMCP tools for AI agent orchestration.
  - **L3 GUI**: High-performance native desktop UI via `egui 0.37`.

## Architecture

- `crates/spendcraft-store`: Pure SQLite database layer with transactional integrity.
- `crates/spendcraft-core`: Statutory travel calculation engine, SEPA pain.001 XML generator, bus handler.
- `crates/spendcraft-mcp`: Stdio JSON-RPC MCP server & CLI binary (`spendcraft`).
- `crates/spendcraft-app`: Native desktop GUI executable (`spendcraft-app`).

## Quick Start

```powershell
# Run smoke tests
.\start.ps1 -Smoke

# Launch native desktop app
.\start.ps1

# Run MCP server on stdio
.\start.ps1 -Mcp
```

## License

MIT OR Apache-2.0
