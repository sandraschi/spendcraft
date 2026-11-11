---
updated: 2026-11-11T01:18:00+01:00
agent: antigravity
status: completed
tags: [spendcraft, expenses, travel, fahrtenbuch, estg, kilometergeld, sepa, craft-biz, webapp]
---

# Handover — SpendCraft

## What Was Done
- Overhauled `spendcraft` from an undercooked stub into a polished sovereign back-office app:
  1. **UI/UX Elevation (`spendcraft-app`)**: Reskinned using the sovereign `craft-biz-ui` design system with 4 views (Claims Inbox, Fahrtenbuch, §26 EStG Engine, SEPA Batches), Austrian ONORM A 1080 currency formatting (`craft-biz::Money`), KPI tiles, tone pills, and Dark/Light mode.
  2. **Domain Hardening (`spendcraft-store` & `spendcraft-core`)**:
     - Complete statutory Austrian Fahrtenbuch (`trips` table) with PKW, Bicycle, Motorcycle, and Mitfahrer surcharges (+€0.05/km).
     - BMF Auslandsreisesätze foreign per diem tables (AT, DE, CH, IT, GB, CZ, HU, US) with pro-rata rules and meal deductions (breakfast 15%, lunch 50%, dinner 50%).
     - Austrian VAT tax key split (U20, U10, U13, NONE) and Kleinbetragsrechnung (§11 Abs 6 UStG < €400) flag.
     - Event bus emission of `expense.approved` with tax keys to `buchcraft` (EKR Class 7) and `reimbursement.batched`.
  3. **Webapp Companion Addon (`webapp/`)**: Scaffolded modern React 18 + Vite 5 + Tailwind 3 web dashboard with claim auditing and calculation engine on reserved fleet ports (11262/11263).
  4. **Fleet Compliance**: Port pair 11262/11263 allocated via `claim_ports.py`, recorded in `WEBAPP_PORTS.md`, and locked in `fleet-start.config.ps1`.
- Clean compilation across all crates with zero warnings (`#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::todo, clippy::unimplemented)]`), 100% passing tests, and green `--smoke` check.

## Last Concrete Action
- Verified `cargo run --manifest-path spendcraft/Cargo.toml -p spendcraft-app -- --smoke` exits with 0 (`smoke: ok (spendcraft-app)`).
- Verified `npm run build` in `spendcraft/webapp` compiles cleanly.

## Next Step
- Pilot live run with Sandra's real receipts and export test `pain.001.001.03` XML batch into George banking.
