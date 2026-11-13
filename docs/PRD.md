# SpendCraft PRD

Status 13 November 2026.

## What it does

- Expense claims and trips in SQLite.
- Approve, reject, convert a trip to a claim, batch reimbursements.
- `calculate_kilometergeld` and `calculate_tagesdiaeten` in integer cents, with tests (car 42 cents/km, `car_2026` at 50 cents/km, full day, partial day, meal deduction).
- `STATUTORY_RATES_AS_OF` names the compiled ruleset `at-estg-26` and says the Verordnung date is not in this repository.
- `calculate_uk_amap` is HMRC approved mileage for tax year 2026 to 2027 (`uk-amap-2026`): 55p per mile for a car or van on the first 10,000 business miles from 6 April 2026, then 25p, motorcycle 24p, bicycle 20p, and 5p per passenger mile for a car or van. The caller passes miles already paid in that tax year. Checked against GOV.UK on 13 November 2026.
- `calculate_us_business_mileage` is the IRS business standard mileage rate for a 2026 trip date (`us-irs-mileage-2026`): 72.5 cents per mile through 30 June 2026, then 76 cents. Charity and medical rates are not this function. Checked against irs.gov on 13 November 2026.
- `drain_inbox` turns a complete `travel.logged` event into a submitted claim. There is no draft status. A missing employee, country, or duration is rejected.
- `batch_reimburse` emits `reimbursement.batched` to buchcraft and billcraft. A core test checks both inboxes and the pain.001.001.03 payload.

## What it does not do yet

- Replace Expensify or Pleo.
- A UK or US subsistence table. `BMF_PER_DIEM_RATES` rows for GB and US stay Austrian foreign-travel day rates. They are not HMRC scale rates and they are not GSA per diem.
- Interface translations. That follows Storytold `craft-i18n` when a window needs it. The ruleset id is the jurisdiction switch, separate from the UI language.

## Done when

Mileage packs `uk-amap-2026` and `us-irs-mileage-2026` have their own ids and tests. A subsistence pack still needs its own published table. See `BACKOFFICE_THIN_RUNT_PLAN.md`.
