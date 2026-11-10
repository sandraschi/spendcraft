# SpendCraft — AGENTS.md

Local-first sovereign expense claims, Austrian statutory travel allowances (§26 EStG), and SEPA reimbursement engine.

## Crates

- `crates/spendcraft-store`: Pure SQLite database layer (`ExpenseClaims`, `Trips`, `ReimbursementBatches`).
- `crates/spendcraft-core`: Austrian §26 EStG calculation engine (Kilometergeld, Tagesdiäten), SEPA pain.001 XML generator.
- `crates/spendcraft-mcp`: Stdio JSON-RPC FastMCP server & CLI binary (`spendcraft`).
- `crates/spendcraft-app`: Native desktop GUI executable (`spendcraft-app`) via `egui 0.37`.

## Bus Events

- Inbound: `travel.logged` (TeamCraft)
- Outbound: `expense.approved`, `reimbursement.batched`

## Smoke Test

```powershell
.\start.ps1 -Smoke
```
