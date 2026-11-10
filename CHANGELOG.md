# Changelog

All notable changes to `spendcraft` will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-11-10

### Added
- Pure-Rust 4-crate workspace architecture (`spendcraft-store`, `spendcraft-core`, `spendcraft-mcp`, `spendcraft-app`).
- Austrian statutory travel allowance engine implementing Einkommensteuergesetz (EStG) §26:
  - Kilometergeld: 42¢/km car, 40¢/km bicycle with trip distance logbook tracking.
  - Tagesdiäten: €26.40 full-day domestic rate, pro-rated €2.20/hour for trips exceeding 3 hours, statutory meal deductions (€13.20 lunch, €13.20 dinner).
- ISO 20022 SEPA Credit Transfer (`pain.001.001.03`) XML batch reimbursement generator.
- Event bus integration (`craft-biz::bus::Bus`):
  - Inbound: drains `travel.logged` from `teamcraft`.
  - Outbound: emits `expense.approved` and `reimbursement.batched` to `billcraft\inbox`.
- Stdio FastMCP JSON-RPC server and CLI with 6 tools (`spend_status`, `spend_claim_create`, `spend_claim_list`, `spend_claim_approve`, `spend_trip_log`, `spend_sepa_reimbursement_batch`).
- Native egui 0.37 desktop workstation (`spendcraft-app`) with `--smoke` verification.
- Central fleet launcher `mcp-central-docs\starts\spendcraft-start.bat`.
