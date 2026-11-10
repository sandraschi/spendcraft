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
    pub currency: String,
    pub tax_cents: i64,
    pub claim_date: NaiveDate,
    pub status: ClaimStatus,
    pub receipt_filename: Option<String>,
    pub receipt_sha256: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpendStats {
    pub total_claims: usize,
    pub total_amount_cents: i64,
    pub approved_amount_cents: i64,
    pub reimbursed_amount_cents: i64,
    pub pending_approval_count: usize,
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
            CREATE INDEX IF NOT EXISTS idx_claims_employee ON expense_claims(employee_id);",
        )?;
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
        let now = Utc::now();
        let claim = ExpenseClaim {
            id: id.to_string(),
            employee_id: employee_id.to_string(),
            employee_name: employee_name.to_string(),
            employee_iban: employee_iban.map(|s| s.to_string()),
            category: category.to_string(),
            amount_cents,
            currency: "EUR".to_string(),
            tax_cents,
            claim_date,
            status: ClaimStatus::Submitted,
            receipt_filename: receipt_filename.map(|s| s.to_string()),
            receipt_sha256: receipt_sha256.map(|s| s.to_string()),
            notes: notes.map(|s| s.to_string()),
            created_at: now,
        };

        self.conn.execute(
            "INSERT INTO expense_claims (
                id, employee_id, employee_name, employee_iban, category, amount_cents, currency,
                tax_cents, claim_date, status, receipt_filename, receipt_sha256, notes, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                claim.id,
                claim.employee_id,
                claim.employee_name,
                claim.employee_iban,
                claim.category,
                claim.amount_cents,
                claim.currency,
                claim.tax_cents,
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
                    tax_cents, claim_date, status, receipt_filename, receipt_sha256, notes, created_at
             FROM expense_claims WHERE id = ?1",
        )?;

        stmt.query_row(params![id], |row| Self::row_to_claim(row))
            .optional()?
            .ok_or_else(|| SpendStoreError::NotFound(id.to_string()))
    }

    pub fn list_claims(&self, status: Option<ClaimStatus>) -> Result<Vec<ExpenseClaim>, SpendStoreError> {
        let mut sql = "SELECT id, employee_id, employee_name, employee_iban, category, amount_cents, currency,
                              tax_cents, claim_date, status, receipt_filename, receipt_sha256, notes, created_at
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

        Ok(ExpenseClaim {
            id: row.get(0)?,
            employee_id: row.get(1)?,
            employee_name: row.get(2)?,
            employee_iban: row.get(3)?,
            category: row.get(4)?,
            amount_cents: row.get(5)?,
            currency: row.get(6)?,
            tax_cents: row.get(7)?,
            claim_date,
            status,
            receipt_filename: row.get(10)?,
            receipt_sha256: row.get(11)?,
            notes: row.get(12)?,
            created_at,
        })
    }

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

        let (total_claims, total_amount_cents, approved_amount_cents, reimbursed_amount_cents, pending_approval_count) =
            stmt.query_row([], |row| {
                Ok((
                    row.get::<_, i64>(0)? as usize,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)? as usize,
                ))
            })?;

        Ok(SpendStats {
            total_claims,
            total_amount_cents,
            approved_amount_cents,
            reimbursed_amount_cents,
            pending_approval_count,
        })
    }

    /// Austrian §26 Z4 EStG Kilometergeld: calculates official tax-free mileage compensation.
    /// Car: 0.42 €/km (standard flat rate) or 0.50 €/km under 2025/2026 reform
    /// Bicycle: 0.40 €/km
    /// Motorcycle: 0.24 €/km
    pub fn calculate_kilometergeld(km: f64, vehicle: &str) -> i64 {
        let rate_cents = match vehicle {
            "car" => 42,
            "car_2026" => 50,
            "bicycle" => 40,
            "motorcycle" => 24,
            _ => 42,
        };
        (km * rate_cents as f64).round() as i64
    }

    /// Austrian §26 Z4 EStG Tagesdiäten (Domestic Business Trips):
    /// Flat rate: 26.40 € for trips exceeding 11 hours.
    /// 3 to 11 hours: 2.20 € per hour (each started hour).
    /// Under 3 hours: 0.00 €.
    /// Deductions for provided meals: 13.20 € (50%) per free lunch or dinner.
    pub fn calculate_tagesdiaeten(hours: f64, free_lunch: bool, free_dinner: bool) -> i64 {
        if hours < 3.0 {
            return 0;
        }

        let base_cents = if hours >= 11.0 {
            2640 // 26.40 EUR
        } else {
            let started_hours = hours.ceil() as i64;
            started_hours * 220 // 2.20 EUR / hr
        };

        let mut deductions = 0i64;
        if free_lunch {
            deductions += 1320;
        }
        if free_dinner {
            deductions += 1320;
        }

        (base_cents - deductions).max(0)
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
        let msg_id = format!("SEPA-CR-EXP-{}", Utc::now().format("%Y%m%d%H%M%S"));

        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<Document xmlns=\"urn:iso:std:iso:20022:tech:xsd:pain.001.001.03\">\n");
        xml.push_str("  <CstmrCdtTrfInitn>\n");
        xml.push_str("    <GrpHdr>\n");
        xml.push_str(&format!("      <MsgId>{msg_id}</MsgId>\n"));
        xml.push_str(&format!("      <CreDtTm>{}</CreDtTm>\n", Utc::now().format("%Y-%m-%dT%H:%M:%SZ")));
        xml.push_str(&format!("      <NbOfTxs>{count}</NbOfTxs>\n"));
        xml.push_str(&format!("      <CtrlSum>{:.2}</CtrlSum>\n", total_cents as f64 / 100.0));
        xml.push_str("      <InitgPty>\n");
        xml.push_str(&format!("        <Nm>{payer_name}</Nm>\n"));
        xml.push_str("      </InitgPty>\n");
        xml.push_str("    </GrpHdr>\n");

        xml.push_str("    <PmtInf>\n");
        xml.push_str(&format!("      <PmtInfId>{msg_id}-01</PmtInfId>\n"));
        xml.push_str("      <PmtMtd>TRF</PmtMtd>\n");
        xml.push_str("      <PmtTpInf>\n");
        xml.push_str("        <SvcLvl><Cd>SEPA</Cd></SvcLvl>\n");
        xml.push_str("      </PmtTpInf>\n");
        xml.push_str(&format!("      <ReqdExctnDt>{}</ReqdExctnDt>\n", Utc::now().date_naive()));
        xml.push_str("      <Dbtr>\n");
        xml.push_str(&format!("        <Nm>{payer_name}</Nm>\n"));
        xml.push_str("      </Dbtr>\n");
        xml.push_str("      <DbtrAcct>\n");
        xml.push_str("        <Id><IBAN>");
        xml.push_str(payer_iban);
        xml.push_str("</IBAN></Id>\n");
        xml.push_str("      </DbtrAcct>\n");
        xml.push_str("      <DbtrAgt>\n");
        xml.push_str("        <FinInstnId><BIC>");
        xml.push_str(payer_bic);
        xml.push_str("</BIC></FinInstnId>\n");
        xml.push_str("      </DbtrAgt>\n");

        for c in claims {
            xml.push_str("      <CdtTrfTxInf>\n");
            xml.push_str("        <PmtId>\n");
            xml.push_str(&format!("          <EndToEndId>{}</EndToEndId>\n", c.id));
            xml.push_str("        </PmtId>\n");
            xml.push_str(&format!("        <Amt><InstdAmt Ccy=\"EUR\">{:.2}</InstdAmt></Amt>\n", c.amount_cents as f64 / 100.0));
            xml.push_str("        <Cdtr>\n");
            xml.push_str(&format!("          <Nm>{}</Nm>\n", c.employee_name));
            xml.push_str("        </Cdtr>\n");
            xml.push_str("        <CdtrAcct>\n");
            xml.push_str("          <Id><IBAN>");
            xml.push_str(c.employee_iban.as_deref().unwrap_or("NOTPROVIDED"));
            xml.push_str("</IBAN></Id>\n");
            xml.push_str("        </CdtrAcct>\n");
            xml.push_str("        <RmtInf>\n");
            xml.push_str(&format!("          <Ustrd>Expense Reimbursement {}</Ustrd>\n", c.id));
            xml.push_str("        </RmtInf>\n");
            xml.push_str("      </CdtTrfTxInf>\n");
        }

        xml.push_str("    </PmtInf>\n");
        xml.push_str("  </CstmrCdtTrfInitn>\n");
        xml.push_str("</Document>\n");

        Ok(xml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_austrian_estg_calculations() {
        // Kilometergeld
        let km_cents = SpendStore::calculate_kilometergeld(100.0, "car");
        assert_eq!(km_cents, 4200); // 42.00 EUR

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
}
