# SpendCraft — AGENTS.md

Local-first sovereign expense claims, Austrian statutory travel allowances (§26 EStG), and SEPA reimbursement engine.

## Crates & Addons

- `crates/spendcraft-store`: SQLite database layer (`expense_claims`, `trips`, `reimbursement_batches`).
- `crates/spendcraft-core`: Austrian §26 EStG calculation engine (Kilometergeld, BMF Auslandsreisesätze, Tagesdiäten), SEPA pain.001 XML generator.
- `crates/spendcraft-mcp`: Stdio JSON-RPC FastMCP server & CLI binary (`spendcraft`).
- `crates/spendcraft-app`: Native executive desktop GUI executable (`spendcraft-app`) via `egui 0.37` and `craft-biz-ui`.
- `webapp/`: Webapp companion dashboard (React 18 + Vite 5 + Tailwind 3).

## Ports

- Frontend (Vite): 11262
- Backend: 11263

## Bus Events

- Inbound: `travel.logged` (TeamCraft)
- Outbound: `expense.approved` (BuchCraft / BillCraft), `reimbursement.batched` (BuchCraft / BillCraft)

## Smoke Test

```powershell
.\start.ps1 -Smoke
```
