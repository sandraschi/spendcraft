#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpendStoreError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Claim not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    Submitted,
    Approved,
    Reimbursed,
    Rejected,
}

impl ClaimStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Approved => "approved",
            Self::Reimbursed => "reimbursed",
            Self::Rejected => "rejected",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "submitted" => Some(Self::Submitted),
            "approved" => Some(Self::Approved),
            "reimbursed" => Some(Self::Reimbursed),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpenseClaim {
    pub id: String,
    pub employee_id: String,
    pub employee_name: String,
    pub employee_iban: Option<String>,
    pub category: String,
    pub amount_cents: i64,
    pub net_cents: i64,
    pub tax_cents: i64,
    pub tax_key: String,
    pub is_kleinbetragsrechnung: bool,
    pub currency: String,
    pub claim_date: NaiveDate,
    pub status: ClaimStatus,
    pub receipt_filename: Option<String>,
    pub receipt_sha256: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trip {
    pub id: String,
    pub employee_id: String,
    pub employee_name: String,
    pub trip_date: NaiveDate,
    pub departure_time: Option<String>,
    pub arrival_time: Option<String>,
    pub origin: String,
    pub destination: String,
    pub purpose: String,
    pub vehicle: String,
    pub start_km: Option<i64>,
    pub end_km: Option<i64>,
    pub distance_km: f64,
    pub passengers: i64,
    pub rate_cents_per_km: i64,
    pub allowance_cents: i64,
    pub claim_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReimbursementBatch {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub total_amount_cents: i64,
    pub claim_count: usize,
    pub status: String,
    pub xml_payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendStats {
    pub total_claims: usize,
    pub total_amount_cents: i64,
    pub approved_amount_cents: i64,
    pub reimbursed_amount_cents: i64,
    pub pending_approval_count: usize,
    pub total_trips: usize,
    pub total_trip_distance_km: f64,
    pub total_mileage_allowance_cents: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PerDiemRates {
    pub country_code: &'static str,
    pub country_name: &'static str,
    pub daily_cents: i64,
    pub hourly_cents: i64,
    pub lunch_deduction_cents: i64,
    pub dinner_deduction_cents: i64,
    pub breakfast_deduction_cents: i64,
}

pub const BMF_PER_DIEM_RATES: &[PerDiemRates] = &[
    PerDiemRates {
        country_code: "AT",
        country_name: "Österreich (Inland)",
        daily_cents: 2640,
        hourly_cents: 220,
        lunch_deduction_cents: 1320,
        dinner_deduction_cents: 1320,
        breakfast_deduction_cents: 396,
    },
    PerDiemRates {
        country_code: "DE",
        country_name: "Deutschland",
        daily_cents: 3530,
        hourly_cents: 294,
        lunch_deduction_cents: 1412,
        dinner_deduction_cents: 1412,
        breakfast_deduction_cents: 530,
    },
    PerDiemRates {
        country_code: "CH",
        country_name: "Schweiz",
        daily_cents: 4240,
        hourly_cents: 353,
        lunch_deduction_cents: 1696,
        dinner_deduction_cents: 1696,
        breakfast_deduction_cents: 636,
    },
    PerDiemRates {
        country_code: "IT",
        country_name: "Italien",
        daily_cents: 3600,
        hourly_cents: 300,
        lunch_deduction_cents: 1440,
        dinner_deduction_cents: 1440,
        breakfast_deduction_cents: 540,
    },
    PerDiemRates {
        country_code: "GB",
        country_name: "Großbritannien",
        daily_cents: 4000,
        hourly_cents: 333,
        lunch_deduction_cents: 1600,
        dinner_deduction_cents: 1600,
        breakfast_deduction_cents: 600,
    },
    PerDiemRates {
        country_code: "CZ",
        country_name: "Tschechien",
        daily_cents: 3200,
        hourly_cents: 267,
        lunch_deduction_cents: 1280,
        dinner_deduction_cents: 1280,
        breakfast_deduction_cents: 480,
    },
    PerDiemRates {
        country_code: "HU",
        country_name: "Ungarn",
        daily_cents: 2900,
        hourly_cents: 242,
        lunch_deduction_cents: 1160,
        dinner_deduction_cents: 1160,
        breakfast_deduction_cents: 435,
    },
    PerDiemRates {
        country_code: "US",
        country_name: "USA",
        daily_cents: 5200,
        hourly_cents: 433,
        lunch_deduction_cents: 2080,
        dinner_deduction_cents: 2080,
        breakfast_deduction_cents: 780,
    },
];

/// Which statutory table this binary was compiled with.
///
/// Domestic Tagesdiäten in [`BMF_PER_DIEM_RATES`] are €26.40 from 11 hours and
/// €2.20 per started hour from 3 hours. `car` is 42 cents/km. `car_2026` is
/// 50 cents/km and is not the default. This repository does not store a BMF
/// Verordnung date for either figure. Confirm the year before a filing.
pub const STATUTORY_RATES_AS_OF: &str =
    "compiled-undated; ruleset=at-estg-26; car=42c/km; car_2026=50c/km; AT-day=2640c";

/// HMRC approved mileage allowance payments for tax year 2026 to 2027.
/// Read from GOV.UK on 13 November 2026. Miles, not kilometres.
pub const UK_AMAP_AS_OF: &str = "checked=2026-11-13; ruleset=uk-amap-2026; source=gov.uk/expenses-and-benefits-business-travel-mileage/rules-for-tax; from=2026-04-06; car-van=55p/mile first 10000 then 25p; motorcycle=24p; bicycle=20p; passenger=5p car-van only";

/// IRS business standard mileage rate for calendar year 2026.
/// Read from irs.gov on 13 November 2026. Miles, not kilometres.
/// Charity and medical rates are a different table and are not in this function.
pub const US_IRS_MILEAGE_AS_OF: &str = "checked=2026-11-13; ruleset=us-irs-mileage-2026; source=irs.gov/tax-professionals/standard-mileage-rates; through=2026-06-30 business=72.5c/mile IR-2025-128; from=2026-07-01 business=76c/mile IR-2026-29";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MileageResult {
    pub ruleset: &'static str,
    pub minor_units: i64,
    pub currency: &'static str,
    pub unit: &'static str,
}

pub struct SpendStore {
    conn: Connection,
}

impl SpendStore {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SpendStoreError> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self, SpendStoreError> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), SpendStoreError> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS expense_claims (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL,
                employee_name TEXT NOT NULL,
                employee_iban TEXT,
                category TEXT NOT NULL,
                amount_cents INTEGER NOT NULL,
                currency TEXT NOT NULL,
                tax_cents INTEGER NOT NULL,
                claim_date TEXT NOT NULL,
                status TEXT NOT NULL,
                receipt_filename TEXT,
                receipt_sha256 TEXT,
                notes TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_claims_status ON expense_claims(status);
            CREATE INDEX IF NOT EXISTS idx_claims_employee ON expense_claims(employee_id);

            CREATE TABLE IF NOT EXISTS trips (
                id TEXT PRIMARY KEY,
                employee_id TEXT NOT NULL,
                employee_name TEXT NOT NULL,
                trip_date TEXT NOT NULL,
                departure_time TEXT,
                arrival_time TEXT,
                origin TEXT NOT NULL,
                destination TEXT NOT NULL,
                purpose TEXT NOT NULL,
                vehicle TEXT NOT NULL,
                start_km INTEGER,
                end_km INTEGER,
                distance_km REAL NOT NULL,
                passengers INTEGER NOT NULL DEFAULT 0,
                rate_cents_per_km INTEGER NOT NULL,
                allowance_cents INTEGER NOT NULL,
                claim_id TEXT,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_trips_employee ON trips(employee_id);
            CREATE INDEX IF NOT EXISTS idx_trips_date ON trips(trip_date);

            CREATE TABLE IF NOT EXISTS reimbursement_batches (
                id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                total_amount_cents INTEGER NOT NULL,
                claim_count INTEGER NOT NULL,
                status TEXT NOT NULL,
                xml_payload TEXT NOT NULL
            );",
        )?;

        // Safe alterations for existing tables
        let _ = self.conn.execute(
            "ALTER TABLE expense_claims ADD COLUMN net_cents INTEGER DEFAULT 0",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE expense_claims ADD COLUMN tax_key TEXT DEFAULT 'U20'",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE expense_claims ADD COLUMN is_kleinbetragsrechnung INTEGER DEFAULT 0",
            [],
        );

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_claim(
        &self,
        id: &str,
        employee_id: &str,
        employee_name: &str,
        employee_iban: Option<&str>,
        category: &str,
        amount_cents: i64,
        tax_cents: i64,
        claim_date: NaiveDate,
        receipt_filename: Option<&str>,
        receipt_sha256: Option<&str>,
        notes: Option<&str>,
    ) -> Result<ExpenseClaim, SpendStoreError> {
        let net_cents = amount_cents - tax_cents;
        let is_kleinbetragsrechnung = amount_cents <= 40000;
        let tax_key = if tax_cents == 0 {
            "NONE".to_string()
        } else {
            "U20".to_string()
        };

        self.submit_claim_extended(
            id,
            employee_id,
            employee_name,
            employee_iban,
            category,
            amount_cents,
            net_cents,
            tax_cents,
            &tax_key,
            is_kleinbetragsrechnung,
            claim_date,
            receipt_filename,
            receipt_sha256,
            notes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit_claim_extended(
        &self,
        id: &str,
        employee_id: &str,
        employee_name: &str,
        employee_iban: Option<&str>,
        category: &str,
        amount_cents: i64,
        net_cents: i64,
        tax_cents: i64,
        tax_key: &str,
        is_kleinbetragsrechnung: bool,
        claim_date: NaiveDate,
        receipt_filename: Option<&str>,
        receipt_sha256: Option<&str>,
        notes: Option<&str>,
    ) -> Result<ExpenseClaim, SpendStoreError> {
        let now = Utc::now();
        let claim = ExpenseClaim {
            id: id.to_string(),
            employee_id: employee_id.to_string(),
            employee_name: employee_name.to_string(),
            employee_iban: employee_iban.map(|s| s.to_string()),
            category: category.to_string(),
            amount_cents,
            net_cents,
            tax_cents,
            tax_key: tax_key.to_string(),
            is_kleinbetragsrechnung,
            currency: "EUR".to_string(),
            claim_date,
            status: ClaimStatus::Submitted,
            receipt_filename: receipt_filename.map(|s| s.to_string()),
            receipt_sha256: receipt_sha256.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
            created_at: now,
        };

        self.conn.execute(
            "INSERT INTO expense_claims (
                id, employee_id, employee_name, employee_iban, category, amount_cents, net_cents,
                tax_cents, tax_key, is_kleinbetragsrechnung, currency, claim_date, status,
                receipt_filename, receipt_sha256, notes, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                claim.id,
                claim.employee_id,
                claim.employee_name,
                claim.employee_iban,
                claim.category,
                claim.amount_cents,
                claim.net_cents,
                claim.tax_cents,
                claim.tax_key,
                claim.is_kleinbetragsrechnung as i32,
                claim.currency,
                claim.claim_date.to_string(),
                claim.status.as_str(),
                claim.receipt_filename,
                claim.receipt_sha256,
                claim.notes,
                claim.created_at.to_rfc3339()
            ],
        )?;

        Ok(claim)
    }

    pub fn get_claim(&self, id: &str) -> Result<ExpenseClaim, SpendStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, employee_id, employee_name, employee_iban, category, amount_cents, currency,
                    tax_cents, claim_date, status, receipt_filename, receipt_sha256, notes, created_at,
                    COALESCE(net_cents, 0), COALESCE(tax_key, 'U20'), COALESCE(is_kleinbetragsrechnung, 0)
             FROM expense_claims WHERE id = ?1",
        )?;

        stmt.query_row(params![id], Self::row_to_claim)
            .optional()?
            .ok_or_else(|| SpendStoreError::NotFound(id.to_string()))
    }

    pub fn list_claims(
        &self,
        status: Option<ClaimStatus>,
    ) -> Result<Vec<ExpenseClaim>, SpendStoreError> {
        let mut sql = "SELECT id, employee_id, employee_name, employee_iban, category, amount_cents, currency,
                              tax_cents, claim_date, status, receipt_filename, receipt_sha256, notes, created_at,
                              COALESCE(net_cents, 0), COALESCE(tax_key, 'U20'), COALESCE(is_kleinbetragsrechnung, 0)
                       FROM expense_claims".to_string();
        if status.is_some() {
            sql.push_str(" WHERE status = ?1");
        }
        sql.push_str(" ORDER BY claim_date DESC, id DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let mut out = Vec::new();
        if let Some(st) = status {
            let rows = stmt.query_map(params![st.as_str()], Self::row_to_claim)?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let rows = stmt.query_map([], Self::row_to_claim)?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    }

    pub fn update_status(&self, id: &str, new_status: ClaimStatus) -> Result<(), SpendStoreError> {
        let rows = self.conn.execute(
            "UPDATE expense_claims SET status = ?1 WHERE id = ?2",
            params![new_status.as_str(), id],
        )?;
        if rows == 0 {
            return Err(SpendStoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    fn row_to_claim(row: &rusqlite::Row) -> Result<ExpenseClaim, rusqlite::Error> {
        let status_str: String = row.get(9)?;
        let status = ClaimStatus::from_str_opt(&status_str).unwrap_or(ClaimStatus::Submitted);
        let date_str: String = row.get(8)?;
        let created_str: String = row.get(13)?;

        let claim_date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default());
        let created_at = DateTime::parse_from_rfc3339(&created_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let amount_cents: i64 = row.get(5)?;
        let tax_cents: i64 = row.get(7)?;
        let net_cents_raw: i64 = row.get(14)?;
        let net_cents = if net_cents_raw > 0 {
            net_cents_raw
        } else {
            amount_cents - tax_cents
        };
        let tax_key: String = row.get(15)?;
        let is_kb: i32 = row.get(16)?;

        Ok(ExpenseClaim {
            id: row.get(0)?,
            employee_id: row.get(1)?,
            employee_name: row.get(2)?,
            employee_iban: row.get(3)?,
            category: row.get(4)?,
            amount_cents,
            net_cents,
            tax_cents,
            tax_key,
            is_kleinbetragsrechnung: is_kb != 0,
            currency: row.get(6)?,
            claim_date,
            status,
            receipt_filename: row.get(10)?,
            receipt_sha256: row.get(11)?,
            notes: row.get(12)?,
            created_at,
        })
    }

    // --- Fahrtenbuch (Trips) ---

    #[allow(clippy::too_many_arguments)]
    pub fn record_trip(
        &self,
        id: &str,
        employee_id: &str,
        employee_name: &str,
        trip_date: NaiveDate,
        departure_time: Option<&str>,
        arrival_time: Option<&str>,
        origin: &str,
        destination: &str,
        purpose: &str,
        vehicle: &str,
        start_km: Option<i64>,
        end_km: Option<i64>,
        distance_km: f64,
        passengers: i64,
    ) -> Result<Trip, SpendStoreError> {
        let (rate_cents, allowance_cents) =
            Self::calculate_trip_allowance(distance_km, vehicle, passengers);
        let now = Utc::now();

        let trip = Trip {
            id: id.to_string(),
            employee_id: employee_id.to_string(),
            employee_name: employee_name.to_string(),
            trip_date,
            departure_time: departure_time.map(|s| s.to_string()),
            arrival_time: arrival_time.map(|s| s.to_string()),
            origin: origin.to_string(),
            destination: destination.to_string(),
            purpose: purpose.to_string(),
            vehicle: vehicle.to_string(),
            start_km,
            end_km,
            distance_km,
            passengers,
            rate_cents_per_km: rate_cents,
            allowance_cents,
            claim_id: None,
            created_at: now,
        };

        self.conn.execute(
            "INSERT INTO trips (
                id, employee_id, employee_name, trip_date, departure_time, arrival_time,
                origin, destination, purpose, vehicle, start_km, end_km, distance_km,
                passengers, rate_cents_per_km, allowance_cents, claim_id, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                trip.id,
                trip.employee_id,
                trip.employee_name,
                trip.trip_date.to_string(),
                trip.departure_time,
                trip.arrival_time,
                trip.origin,
                trip.destination,
                trip.purpose,
                trip.vehicle,
                trip.start_km,
                trip.end_km,
                trip.distance_km,
                trip.passengers,
                trip.rate_cents_per_km,
                trip.allowance_cents,
                trip.claim_id,
                trip.created_at.to_rfc3339()
            ],
        )?;

        Ok(trip)
    }

    pub fn list_trips(&self, employee_id: Option<&str>) -> Result<Vec<Trip>, SpendStoreError> {
        let mut sql =
            "SELECT id, employee_id, employee_name, trip_date, departure_time, arrival_time,
                              origin, destination, purpose, vehicle, start_km, end_km, distance_km,
                              passengers, rate_cents_per_km, allowance_cents, claim_id, created_at
                       FROM trips"
                .to_string();
        if employee_id.is_some() {
            sql.push_str(" WHERE employee_id = ?1");
        }
        sql.push_str(" ORDER BY trip_date DESC, id DESC");

        let mut stmt = self.conn.prepare(&sql)?;
        let mut out = Vec::new();

        let mapper = |row: &rusqlite::Row| -> Result<Trip, rusqlite::Error> {
            let date_str: String = row.get(3)?;
            let created_str: String = row.get(17)?;
            let trip_date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
                .unwrap_or_else(|_| NaiveDate::from_ymd_opt(2026, 1, 1).unwrap_or_default());
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            Ok(Trip {
                id: row.get(0)?,
                employee_id: row.get(1)?,
                employee_name: row.get(2)?,
                trip_date,
                departure_time: row.get(4)?,
                arrival_time: row.get(5)?,
                origin: row.get(6)?,
                destination: row.get(7)?,
                purpose: row.get(8)?,
                vehicle: row.get(9)?,
                start_km: row.get(10)?,
                end_km: row.get(11)?,
                distance_km: row.get(12)?,
                passengers: row.get(13)?,
                rate_cents_per_km: row.get(14)?,
                allowance_cents: row.get(15)?,
                claim_id: row.get(16)?,
                created_at,
            })
        };

        if let Some(emp) = employee_id {
            let rows = stmt.query_map(params![emp], mapper)?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let rows = stmt.query_map([], mapper)?;
            for r in rows {
                out.push(r?);
            }
        }

        Ok(out)
    }

    pub fn delete_trip(&self, id: &str) -> Result<(), SpendStoreError> {
        let rows = self
            .conn
            .execute("DELETE FROM trips WHERE id = ?1", params![id])?;
        if rows == 0 {
            return Err(SpendStoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    // --- Stats & Calculations ---

    pub fn stats(&self) -> Result<SpendStats, SpendStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT 
                COUNT(*),
                COALESCE(SUM(amount_cents), 0),
                COALESCE(SUM(CASE WHEN status IN ('approved', 'reimbursed') THEN amount_cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'reimbursed' THEN amount_cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'submitted' THEN 1 ELSE 0 END), 0)
             FROM expense_claims",
        )?;

        let (
            total_claims,
            total_amount_cents,
            approved_amount_cents,
            reimbursed_amount_cents,
            pending_approval_count,
        ) = stmt.query_row([], |row| {
            Ok((
                row.get::<_, i64>(0)? as usize,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)? as usize,
            ))
        })?;

        let mut trip_stmt = self.conn.prepare(
            "SELECT COUNT(*), COALESCE(SUM(distance_km), 0.0), COALESCE(SUM(allowance_cents), 0) FROM trips",
        )?;

        let (total_trips, total_trip_distance_km, total_mileage_allowance_cents) = trip_stmt
            .query_row([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, f64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;

        Ok(SpendStats {
            total_claims,
            total_amount_cents,
            approved_amount_cents,
            reimbursed_amount_cents,
            pending_approval_count,
            total_trips,
            total_trip_distance_km,
            total_mileage_allowance_cents,
        })
    }

    /// Austrian §26 Z 4 Kilometergeld, in cents.
    ///
    /// See [`STATUTORY_RATES_AS_OF`]. `car` is 42 c/km. `car_2026` is 50 c/km
    /// and is not selected unless the caller names it. Bicycle is 40 c/km,
    /// motorcycle is 24 c/km, and each passenger adds 5 c/km. An unknown
    /// vehicle uses the `car` row.
    pub fn calculate_kilometergeld(km: f64, vehicle: &str) -> i64 {
        let (rate, allowance) = Self::calculate_trip_allowance(km, vehicle, 0);
        let _ = rate;
        allowance
    }

    pub fn calculate_trip_allowance(km: f64, vehicle: &str, passengers: i64) -> (i64, i64) {
        let base_rate_cents = match vehicle {
            "car" => 42,
            "car_2026" => 50,
            "bicycle" => 40,
            "motorcycle" => 24,
            _ => 42,
        };
        let passenger_surcharge = passengers.max(0) * 5;
        let total_rate_cents = base_rate_cents + passenger_surcharge;
        let total_allowance = (km * total_rate_cents as f64).round() as i64;
        (total_rate_cents, total_allowance)
    }

    /// HMRC approved mileage for tax year 2026 to 2027, in pence.
    ///
    /// `miles_already` is business miles of that vehicle class already paid
    /// in the same tax year. The 10,000-mile step is on the year, not the trip.
    /// A passenger payment is 5p per mile and only for a car or van.
    /// See [`UK_AMAP_AS_OF`].
    pub fn calculate_uk_amap(
        miles: f64,
        vehicle: &str,
        passengers: i64,
        miles_already: f64,
    ) -> Result<MileageResult, SpendStoreError> {
        if miles < 0.0 || miles_already < 0.0 || passengers < 0 {
            return Err(SpendStoreError::Validation(
                "miles, miles already claimed, and passengers must be zero or more".into(),
            ));
        }
        let class = match vehicle {
            "car" | "van" => "car",
            "motorcycle" => "motorcycle",
            "bicycle" | "cycle" => "bicycle",
            other => {
                return Err(SpendStoreError::Validation(format!(
                    "uk-amap-2026 has no vehicle {other}"
                )))
            }
        };
        if passengers > 0 && class != "car" {
            return Err(SpendStoreError::Validation(
                "a passenger payment applies to a car or van only".into(),
            ));
        }
        let pence = match class {
            "car" => {
                let room = (10_000.0 - miles_already).max(0.0);
                let at_first = miles.min(room);
                let at_rest = miles - at_first;
                minor_units(at_first, 55)
                    + minor_units(at_rest, 25)
                    + minor_units(miles, passengers * 5)
            }
            "motorcycle" => minor_units(miles, 24),
            _ => minor_units(miles, 20),
        };
        Ok(MileageResult {
            ruleset: "uk-amap-2026",
            minor_units: pence,
            currency: "GBP",
            unit: "mile",
        })
    }

    /// IRS business standard mileage for a 2026 trip date, in cents.
    ///
    /// 72.5 cents per mile through 30 June 2026, then 76 cents. A half-cent
    /// rounds away from zero. A date outside 2026 is rejected. See [`US_IRS_MILEAGE_AS_OF`].
    pub fn calculate_us_business_mileage(
        miles: f64,
        on: NaiveDate,
    ) -> Result<MileageResult, SpendStoreError> {
        if miles < 0.0 {
            return Err(SpendStoreError::Validation(
                "miles must be zero or more".into(),
            ));
        }
        let Some(year_start) = NaiveDate::from_ymd_opt(2026, 1, 1) else {
            return Err(SpendStoreError::Validation("bad date".into()));
        };
        let Some(second_half) = NaiveDate::from_ymd_opt(2026, 7, 1) else {
            return Err(SpendStoreError::Validation("bad date".into()));
        };
        let Some(year_end) = NaiveDate::from_ymd_opt(2026, 12, 31) else {
            return Err(SpendStoreError::Validation("bad date".into()));
        };
        if on < year_start || on > year_end {
            return Err(SpendStoreError::Validation(
                "us-irs-mileage-2026 covers trip dates in 2026 only".into(),
            ));
        }
        let mills_per_mile: i64 = if on < second_half { 725 } else { 760 };
        let mills = (miles * mills_per_mile as f64).round() as i64;
        let cents = if mills >= 0 { (mills + 5) / 10 } else { 0 };
        Ok(MileageResult {
            ruleset: "us-irs-mileage-2026",
            minor_units: cents,
            currency: "USD",
            unit: "mile",
        })
    }

    /// Austrian §26 Z4 EStG Tagesdiäten (Domestic Business Trips):
    /// Flat rate: 26.40 € for trips >= 11 hours (or full day 12h).
    /// 3 to 11 hours: 2.20 € per hour (each started hour).
    /// Under 3 hours: 0.00 €.
    /// Deductions for provided meals: 13.20 € (50%) per free lunch or dinner.
    pub fn calculate_tagesdiaeten(hours: f64, free_lunch: bool, free_dinner: bool) -> i64 {
        Self::calculate_foreign_per_diem("AT", hours, false, free_lunch, free_dinner)
    }

    /// BMF Auslandsreisesätze calculation:
    /// Supports Austria and foreign countries (DE, CH, IT, GB, CZ, HU, US).
    /// Deductions:
    /// - Free breakfast: ~15% deduction
    /// - Free lunch: 50% deduction
    /// - Free dinner: 50% deduction
    pub fn calculate_foreign_per_diem(
        country: &str,
        hours: f64,
        free_breakfast: bool,
        free_lunch: bool,
        free_dinner: bool,
    ) -> i64 {
        if hours < 3.0 {
            return 0;
        }

        let rates = BMF_PER_DIEM_RATES
            .iter()
            .find(|r| r.country_code.eq_ignore_ascii_case(country))
            .copied()
            .unwrap_or(BMF_PER_DIEM_RATES[0]);

        let base_cents = if hours >= 11.0 {
            rates.daily_cents
        } else {
            let started_hours = hours.ceil() as i64;
            started_hours * rates.hourly_cents
        };

        let mut deductions = 0i64;
        if free_breakfast {
            deductions += rates.breakfast_deduction_cents;
        }
        if free_lunch {
            deductions += rates.lunch_deduction_cents;
        }
        if free_dinner {
            deductions += rates.dinner_deduction_cents;
        }

        (base_cents - deductions).max(0)
    }

    // --- SEPA Batch Management ---

    pub fn list_batches(&self) -> Result<Vec<ReimbursementBatch>, SpendStoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, created_at, total_amount_cents, claim_count, status, xml_payload
             FROM reimbursement_batches ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let created_str: String = row.get(1)?;
            let created_at = DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());
            Ok(ReimbursementBatch {
                id: row.get(0)?,
                created_at,
                total_amount_cents: row.get(2)?,
                claim_count: row.get::<_, i64>(3)? as usize,
                status: row.get(4)?,
                xml_payload: row.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Generates ISO 20022 pain.001.001.03 SEPA Credit Transfer XML for batch reimbursement.
    pub fn export_sepa_reimbursement(
        &self,
        claim_ids: &[String],
        payer_name: &str,
        payer_iban: &str,
        payer_bic: &str,
    ) -> Result<String, SpendStoreError> {
        let mut claims = Vec::new();
        for id in claim_ids {
            claims.push(self.get_claim(id)?);
        }

        let total_cents: i64 = claims.iter().map(|c| c.amount_cents).sum();
        let count = claims.len();
        let batch_id = format!("BATCH-{}", Utc::now().format("%Y%m%d%H%M%S"));
        let msg_id = format!("SEPA-CR-EXP-{}", Utc::now().format("%Y%m%d%H%M%S"));

        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.001.001.03\">\n");
        xml.push_str("  <CstmrCdtTrfInitn>\n");
        xml.push_str("    <GrpHdr>\n");
        xml.push_str(&format!("      <MsgId>{}</MsgId>\n", msg_id));
        xml.push_str(&format!(
            "      <CreDtTm>{}</CreDtTm>\n",
            Utc::now().to_rfc3339()
        ));
        xml.push_str(&format!("      <NbOfTxs>{}</NbOfTxs>\n", count));
        xml.push_str(&format!(
            "      <CtrlSum>{:.2}</CtrlSum>\n",
            total_cents as f64 / 100.0
        ));
        xml.push_str("      <InitgPty>\n");
        xml.push_str(&format!("        <Nm>{}</Nm>\n", payer_name));
        xml.push_str("      </InitgPty>\n");
        xml.push_str("    </GrpHdr>\n");
        xml.push_str("    <PmtInf>\n");
        xml.push_str(&format!("      <PmtInfId>PMT-{}</PmtInfId>\n", msg_id));
        xml.push_str("      <PmtMtd>TRF</PmtMtd>\n");
        xml.push_str(&format!("      <NbOfTxs>{}</NbOfTxs>\n", count));
        xml.push_str(&format!(
            "      <CtrlSum>{:.2}</CtrlSum>\n",
            total_cents as f64 / 100.0
        ));
        xml.push_str("      <PmtTpInf>\n");
        xml.push_str("        <SvcLvl><Cd>SEPA</Cd></SvcLvl>\n");
        xml.push_str("      </PmtTpInf>\n");
        xml.push_str(&format!(
            "      <ReqdExctnDt>{}</ReqdExctnDt>\n",
            Utc::now().format("%Y-%m-%d")
        ));
        xml.push_str("      <Dbtr>\n");
        xml.push_str(&format!("        <Nm>{}</Nm>\n", payer_name));
        xml.push_str("      </Dbtr>\n");
        xml.push_str("      <DbtrAcct>\n");
        xml.push_str(&format!("        <Id><IBAN>{}</IBAN></Id>\n", payer_iban));
        xml.push_str("      </DbtrAcct>\n");
        xml.push_str("      <DbtrAgt>\n");
        xml.push_str("        <FinInstnId><BIC>");
        xml.push_str(payer_bic);
        xml.push_str("</BIC></FinInstnId>\n");
        xml.push_str("      </DbtrAgt>\n");

        for c in &claims {
            xml.push_str("      <CdtTrfTxInf>\n");
            xml.push_str("        <PmtId>\n");
            xml.push_str(&format!("          <EndToEndId>{}</EndToEndId>\n", c.id));
            xml.push_str("        </PmtId>\n");
            xml.push_str(&format!(
                "        <Amt><InstdAmt Ccy=\"EUR\">{:.2}</InstdAmt></Amt>\n",
                c.amount_cents as f64 / 100.0
            ));
            xml.push_str("        <Cdtr>\n");
            xml.push_str(&format!("          <Nm>{}</Nm>\n", c.employee_name));
            xml.push_str("        </Cdtr>\n");
            xml.push_str("        <CdtrAcct>\n");
            xml.push_str("          <Id><IBAN>");
            xml.push_str(c.employee_iban.as_deref().unwrap_or("NOTPROVIDED"));
            xml.push_str("</IBAN></Id>\n");
            xml.push_str("        </CdtrAcct>\n");
            xml.push_str("        <RmtInf>\n");
            xml.push_str(&format!(
                "          <Ustrd>Expense Reimbursement {}</Ustrd>\n",
                c.id
            ));
            xml.push_str("        </RmtInf>\n");
            xml.push_str("      </CdtTrfTxInf>\n");
        }

        xml.push_str("    </PmtInf>\n");
        xml.push_str("  </CstmrCdtTrfInitn>\n");
        xml.push_str("</Document>\n");

        // Record batch in DB
        let _ = self.conn.execute(
            "INSERT INTO reimbursement_batches (id, created_at, total_amount_cents, claim_count, status, xml_payload)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                batch_id,
                Utc::now().to_rfc3339(),
                total_cents,
                count as i64,
                "exported",
                xml
            ],
        );

        Ok(xml)
    }
}

fn minor_units(distance: f64, rate: i64) -> i64 {
    (distance * rate as f64).round() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_austrian_estg_calculations() {
        assert!(STATUTORY_RATES_AS_OF.contains("compiled-undated"));
        assert!(STATUTORY_RATES_AS_OF.contains("ruleset=at-estg-26"));
        assert!(STATUTORY_RATES_AS_OF.contains("car=42c/km"));

        // Kilometergeld. The default car row is 42 c/km. car_2026 is a separate row.
        let km_cents = SpendStore::calculate_kilometergeld(100.0, "car");
        assert_eq!(km_cents, 4200);
        assert_eq!(SpendStore::calculate_kilometergeld(100.0, "car_2026"), 5000);

        // Tagesdiäten full day
        let diaeten_full = SpendStore::calculate_tagesdiaeten(12.0, false, false);
        assert_eq!(diaeten_full, 2640); // 26.40 EUR

        // Tagesdiäten pro-rated 5 hours
        let diaeten_5h = SpendStore::calculate_tagesdiaeten(5.0, false, false);
        assert_eq!(diaeten_5h, 1100); // 5h * 2.20 = 11.00 EUR

        // With free lunch provided
        let diaeten_lunch = SpendStore::calculate_tagesdiaeten(12.0, true, false);
        assert_eq!(diaeten_lunch, 1320); // 26.40 - 13.20 = 13.20 EUR
    }

    #[test]
    fn test_trip_allowance_with_passengers() {
        // 100km by car with 2 passengers: (42 + 2 * 5) = 52c/km -> 52.00 EUR
        let (rate, total) = SpendStore::calculate_trip_allowance(100.0, "car", 2);
        assert_eq!(rate, 52);
        assert_eq!(total, 5200);
    }

    #[test]
    fn test_bmf_foreign_rates() {
        // Germany: 35.30 EUR full day
        let de_full = SpendStore::calculate_foreign_per_diem("DE", 12.0, false, false, false);
        assert_eq!(de_full, 3530);

        // Germany with free lunch (14.12 EUR deduction)
        let de_lunch = SpendStore::calculate_foreign_per_diem("DE", 12.0, false, true, false);
        assert_eq!(de_lunch, 3530 - 1412);

        // Switzerland: 42.40 EUR full day
        let ch_full = SpendStore::calculate_foreign_per_diem("CH", 12.0, false, false, false);
        assert_eq!(ch_full, 4240);
    }

    #[test]
    fn uk_amap_2026_matches_the_published_worked_example() -> Result<(), String> {
        assert!(UK_AMAP_AS_OF.contains("ruleset=uk-amap-2026"));
        assert!(UK_AMAP_AS_OF.contains("55p"));
        let year =
            SpendStore::calculate_uk_amap(12_000.0, "car", 0, 0.0).map_err(|e| e.to_string())?;
        assert_eq!(year.minor_units, 600_000);
        assert_eq!(year.currency, "GBP");
        let across =
            SpendStore::calculate_uk_amap(2_000.0, "van", 0, 9_000.0).map_err(|e| e.to_string())?;
        assert_eq!(across.minor_units, 80_000);
        let bike =
            SpendStore::calculate_uk_amap(10.0, "bicycle", 0, 0.0).map_err(|e| e.to_string())?;
        assert_eq!(bike.minor_units, 200);
        let with_passenger =
            SpendStore::calculate_uk_amap(10.0, "car", 1, 0.0).map_err(|e| e.to_string())?;
        assert_eq!(with_passenger.minor_units, 600);
        assert!(SpendStore::calculate_uk_amap(10.0, "motorcycle", 1, 0.0).is_err());
        Ok(())
    }

    #[test]
    fn us_irs_2026_business_mileage_follows_the_july_change() -> Result<(), String> {
        assert!(US_IRS_MILEAGE_AS_OF.contains("72.5c"));
        assert!(US_IRS_MILEAGE_AS_OF.contains("76c"));
        let june = NaiveDate::from_ymd_opt(2026, 6, 30).ok_or("bad date")?;
        let july = NaiveDate::from_ymd_opt(2026, 7, 1).ok_or("bad date")?;
        let first =
            SpendStore::calculate_us_business_mileage(100.0, june).map_err(|e| e.to_string())?;
        assert_eq!(first.minor_units, 7_250);
        assert_eq!(first.currency, "USD");
        let second =
            SpendStore::calculate_us_business_mileage(100.0, july).map_err(|e| e.to_string())?;
        assert_eq!(second.minor_units, 7_600);
        let before = NaiveDate::from_ymd_opt(2025, 12, 31).ok_or("bad date")?;
        assert!(SpendStore::calculate_us_business_mileage(10.0, before).is_err());
        Ok(())
    }
}
