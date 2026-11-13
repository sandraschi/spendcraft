# Changelog — SpendCraft

## [Unreleased] - 2026-11-13

### Added
- The window menu is File, Edit, View, Window, and Help. View switches the four tabs. `--smoke` checks the menu.
- `uk-amap-2026`: HMRC approved mileage for tax year 2026 to 2027. Car or van 55p per mile for the first 10,000 business miles from 6 April 2026, then 25p. Motorcycle 24p. Bicycle 20p. Passenger 5p per mile for a car or van. CLI `calc-uk`.
- `us-irs-mileage-2026`: IRS business standard mileage for a 2026 trip date. 72.5 cents per mile through 30 June 2026, 76 cents from 1 July 2026. CLI `calc-us`.

## [0.2.0] - 2026-11-11

### Added
- **Native GUI Overhaul**: Integrated `craft-biz-ui` sovereign design system into `spendcraft-app`:
  - 4 dedicated views: Claims Inbox, Fahrtenbuch (Trips), §26 EStG Engine, SEPA pain.001 Batches.
  - KPI tiles, high-contrast tone pills, and Austrian ONORM A 1080 currency formatting via `craft-biz::Money`.
  - Light/Dark mode toggle with persistent tokens.
- **Austrian Statutory Fahrtenbuch**:
  - `trips` table with start/destination, odometer, purpose, vehicle class, and passenger tracking.
  - Passenger surcharge (*Mitbeförderungszuschlag* +€0.05/km).
- **BMF Foreign Per Diem Rates (Auslandsreisesätze)**:
  - Rates for AT, DE, CH, IT, GB, CZ, HU, US with statutory deductions (breakfast 15%, lunch 50%, dinner 50%).
- **Austrian VAT / UStG Compliance**:
  - Tax keys `U20`, `U10`, `U13`, `NONE` with automatic *Kleinbetragsrechnung* check (< €400).
- **Webapp Companion Addon**:
  - React + Vite + Tailwind dashboard in `webapp/` running on registered fleet ports 11262/11263.
- **Fleet Contract**: Added `fleet-start.config.ps1` adhering to `operations/fleet.lock.json`.

## [0.1.0] - 2026-11-10

- Initial release with `spendcraft-store`, `spendcraft-core`, `spendcraft-mcp`, and minimal `spendcraft-app`.
