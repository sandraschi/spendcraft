# SpendCraft 💳

**Local-first sovereign expense claims, Austrian statutory travel allowances (§26 EStG), and SEPA reimbursement engine.**

Part of the Sovereign Back-Office Suite replacing SaaS spend with zero-cloud-dependency, instant, offline-capable Rust native applications and companion dashboards.

## Features

- **Austrian Statutory Travel Allowance Engine (§26 EStG)**:
  - **Fahrtenbuch (Audit-Compliant Logbook)**: Complete departure/arrival tracking, origin/destination routes, vehicle classes (PKW €0.42/km, 2026 reform €0.50/km, Bicycle €0.40/km, Motorcycle €0.24/km), and statutory passenger surcharges (*Mitbeförderungszuschlag* +€0.05/km).
  - **Tagesdiäten (Domestic & BMF Foreign Per Diems)**:
    - Domestic (AT): €26.40 full-day, pro-rated €2.20/hour for trips > 3 hours.
    - Foreign (BMF Auslandsreisesätze): DE (€35.30), CH (€42.40), IT (€36.00), GB (€40.00), CZ (€32.00), HU (€29.00), US (€52.00).
    - Statutory deductions for provided meals: breakfast (15%), lunch (50%), dinner (50%).
- **Austrian VAT / UStG Compliance**:
  - Input VAT split: `U20` (20%), `U10` (10%), `U13` (13%), `NONE` (0% / exempt).
  - Automatic *Kleinbetragsrechnung* verification (§11 Abs 6 UStG < €400).
- **ISO 20022 SEPA Credit Transfer (`pain.001.001.03`)**: Generates compliant XML reimbursement batch payment orders directly importable into George (Erste Bank), ELBA (Raiffeisen), or Bank Austria.
- **Event Bus Integration**: Consumes `travel.logged` from `teamcraft`, emits `expense.approved` and `reimbursement.batched` to `buchcraft` (EKR Class 7) and `billcraft`.
- **Multi-Modal Interface**:
  - **L1 CLI**: Quick claim logging and command-line approval.
  - **L2 MCP**: Stdio FastMCP tools for AI agent orchestration.
  - **L3 Native Desktop Workstation**: High-performance executive desktop UI via `egui 0.37` and `craft-biz-ui` design system.
  - **Webapp Companion Addon**: Modern React + Vite + Tailwind dashboard running on fleet ports 11262 (Frontend) / 11263 (Backend).

## Architecture

- `crates/spendcraft-store`: SQLite database layer (`expense_claims`, `trips`, `reimbursement_batches`).
- `crates/spendcraft-core`: Austrian §26 EStG calculation engine, SEPA pain.001 XML generator, bus handler.
- `crates/spendcraft-mcp`: Stdio JSON-RPC FastMCP server & CLI binary (`spendcraft`).
- `crates/spendcraft-app`: Native executive desktop GUI executable (`spendcraft-app`) using `craft-biz-ui`.
- `webapp/`: Companion browser dashboard (React, Vite, Tailwind CSS, Lucide icons) on port 11262.

## Quick Start

```powershell
# Run smoke tests
.\start.ps1 -Smoke

# Launch native desktop app
.\start.ps1

# Launch webapp companion
cd webapp; .\start.ps1

# Run MCP server on stdio
.\start.ps1 -Mcp
```

## Ports

- Frontend (Vite): `11262`
- Backend: `11263`

## License

MIT OR Apache-2.0
