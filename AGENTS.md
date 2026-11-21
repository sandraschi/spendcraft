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


## Documentation preservation and hooks

On 2026-11-20 a README "standardization" sweep across the fleet deleted project descriptions,
status sections and user warnings in nine repos. Full rules:
`mcp-central-docs/standards/CONTENT_PRESERVATION_STANDARD.md`.

- Applying any standard or template restructures; it never deletes. Move content (within the file
  or to `docs/` with a link), do not drop it. Never remove what the project is, what it is based on,
  its status (what works, what does not, what is not built) or warnings to users.
- Remove content only when Sandra explicitly asks for that removal, in this repo.
- Never add claims you have not verified (release downloads, versions, test counts, features).
- Only standards with `status: active` may be applied across repos. A change across several repos is
  done on one repo first and approved by Sandra; its commits name the tool and carry a `Sweep-Id:` line.
- `.githooks/commit-msg` blocks a commit that deletes a Markdown file, removes 20% of its lines
  (15 or more) or removes 3 or more headings. A removal Sandra asked for passes with
  `Docs-Removal-Approved: <who and why>` in the message. Enable once per clone:
  `git config core.hooksPath .githooks`. Never use `--no-verify`; if a hook blocks you, stop and ask.
