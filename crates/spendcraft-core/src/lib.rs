#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

use chrono::Local;
use craft_biz::bus::Bus;
use craft_biz::BizError;
use serde::{Deserialize, Serialize};
use serde_json::json;
pub use spendcraft_store::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SpendCoreError {
    #[error("Store error: {0}")]
    Store(#[from] SpendStoreError),
    #[error("Bus error: {0}")]
    Bus(#[from] BizError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

pub struct SpendService {
    store: SpendStore,
    bus: Option<Bus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DrainSummary {
    pub processed: usize,
    pub created_claims: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BatchReimbursementResult {
    pub batch_id: String,
    pub total_amount_cents: i64,
    pub count: usize,
    pub xml_payload: String,
}

impl SpendService {
    pub fn new(store: SpendStore, bus: Option<Bus>) -> Self {
        Self { store, bus }
    }

    pub fn store(&self) -> &SpendStore {
        &self.store
    }

    pub fn bus(&self) -> Option<&Bus> {
        self.bus.as_ref()
    }

    pub fn approve_claim(&self, id: &str) -> Result<ExpenseClaim, SpendCoreError> {
        self.store.update_status(id, ClaimStatus::Approved)?;
        let claim = self.store.get_claim(id)?;

        if let Some(bus) = &self.bus {
            let payload = json!({
                "claim_id": claim.id,
                "employee_id": claim.employee_id,
                "employee_name": claim.employee_name,
                "category": claim.category,
                "amount_cents": claim.amount_cents,
                "net_cents": claim.net_cents,
                "tax_cents": claim.tax_cents,
                "tax_key": claim.tax_key,
                "is_kleinbetragsrechnung": claim.is_kleinbetragsrechnung,
            });
            // Emit to buchcraft (for double-entry ledger Aufwand EKR 7)
            let _ = bus.emit(
                "spendcraft",
                "buchcraft",
                "expense.approved",
                payload.clone(),
            );
            // Emit to billcraft / teamcraft
            let _ = bus.emit("spendcraft", "billcraft", "expense.approved", payload);
        }

        Ok(claim)
    }

    pub fn reject_claim(
        &self,
        id: &str,
        reason: Option<&str>,
    ) -> Result<ExpenseClaim, SpendCoreError> {
        self.store.update_status(id, ClaimStatus::Rejected)?;
        let claim = self.store.get_claim(id)?;

        if let Some(bus) = &self.bus {
            let payload = json!({
                "claim_id": claim.id,
                "employee_id": claim.employee_id,
                "reason": reason.unwrap_or("Claim rejected by audit"),
            });
            let _ = bus.emit("spendcraft", "teamcraft", "expense.rejected", payload);
        }

        Ok(claim)
    }

    /// Converts a trip from the Fahrtenbuch into an approved/submitted expense claim
    pub fn convert_trip_to_claim(&self, trip_id: &str) -> Result<ExpenseClaim, SpendCoreError> {
        let trips = self.store.list_trips(None)?;
        let trip = trips
            .into_iter()
            .find(|t| t.id == trip_id)
            .ok_or_else(|| SpendCoreError::Validation(format!("Trip {trip_id} not found")))?;

        let claim_id = format!("EXP-TRIP-{}", trip.id);
        let notes = format!(
            "Fahrtenbuch Trip {} -> {}: {:.1} km via {} ({} passengers)",
            trip.origin, trip.destination, trip.distance_km, trip.vehicle, trip.passengers
        );

        let claim = self.store.submit_claim_extended(
            &claim_id,
            &trip.employee_id,
            &trip.employee_name,
            None,
            "kilometergeld",
            trip.allowance_cents,
            trip.allowance_cents,
            0,
            "NONE",
            true,
            trip.trip_date,
            None,
            None,
            Some(&notes),
        )?;

        Ok(claim)
    }

    /// Batch reimburses approved claims via ISO 20022 SEPA pain.001.001.03 XML
    pub fn batch_reimburse(
        &self,
        claim_ids: &[String],
        payer_name: &str,
        payer_iban: &str,
        payer_bic: &str,
    ) -> Result<BatchReimbursementResult, SpendCoreError> {
        if claim_ids.is_empty() {
            return Err(SpendCoreError::Validation(
                "No claim IDs provided for batch".into(),
            ));
        }

        let xml = self
            .store
            .export_sepa_reimbursement(claim_ids, payer_name, payer_iban, payer_bic)?;
        let mut total_cents = 0i64;

        for id in claim_ids {
            let claim = self.store.get_claim(id)?;
            total_cents += claim.amount_cents;
            self.store.update_status(id, ClaimStatus::Reimbursed)?;
        }

        let batch_id = format!("BATCH-{}", Local::now().format("%Y%m%d%H%M%S"));

        if let Some(bus) = &self.bus {
            let payload = json!({
                "batch_id": batch_id,
                "claim_ids": claim_ids,
                "total_amount_cents": total_cents,
                "claim_count": claim_ids.len(),
            });
            let _ = bus.emit(
                "spendcraft",
                "buchcraft",
                "reimbursement.batched",
                payload.clone(),
            );
            let _ = bus.emit("spendcraft", "billcraft", "reimbursement.batched", payload);
        }

        Ok(BatchReimbursementResult {
            batch_id,
            total_amount_cents: total_cents,
            count: claim_ids.len(),
            xml_payload: xml,
        })
    }

    /// Automatically generates expense claims for travel events logged by `teamcraft`
    pub fn drain_inbox(&self) -> Result<DrainSummary, SpendCoreError> {
        let mut created = Vec::new();
        let Some(bus) = &self.bus else {
            return Ok(DrainSummary {
                processed: 0,
                created_claims: created,
            });
        };

        let events = bus.pending("spendcraft")?;
        let processed = events.len();

        for ev in events {
            if ev.kind == "travel.logged" {
                let emp_id = ev
                    .payload
                    .get("employee_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("EMP-01");
                let emp_name = ev
                    .payload
                    .get("employee_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Team Member");
                let hours = ev
                    .payload
                    .get("hours")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(8.0);
                let km = ev.payload.get("km").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let free_lunch = ev
                    .payload
                    .get("free_lunch")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let free_dinner = ev
                    .payload
                    .get("free_dinner")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let country = ev
                    .payload
                    .get("country")
                    .and_then(|v| v.as_str())
                    .unwrap_or("AT");

                let per_diem = SpendStore::calculate_foreign_per_diem(
                    country,
                    hours,
                    false,
                    free_lunch,
                    free_dinner,
                );
                let mileage = SpendStore::calculate_kilometergeld(km, "car");
                let total = per_diem + mileage;

                let claim_id = format!("EXP-TRV-{}", Local::now().format("%Y%m%d%H%M%S"));
                let today = Local::now().date_naive();

                if total > 0 && self.store.get_claim(&claim_id).is_err() {
                    let _ = self.store.submit_claim_extended(
                        &claim_id,
                        emp_id,
                        emp_name,
                        None,
                        "travel_per_diem",
                        total,
                        total,
                        0,
                        "NONE",
                        true,
                        today,
                        None,
                        None,
                        Some(&format!(
                            "Automated Austrian §26 EStG calculation: {hours}h travel in {country} (€{:.2}), {km}km (€{:.2})",
                            per_diem as f64 / 100.0,
                            mileage as f64 / 100.0
                        )),
                    );
                    created.push(claim_id);
                }
            }
            let _ = bus.ack("spendcraft", &ev.id);
        }

        Ok(DrainSummary {
            processed,
            created_claims: created,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spend_service_workflow() -> Result<(), Box<dyn std::error::Error>> {
        let store = SpendStore::open_in_memory()?;
        let service = SpendService::new(store, None);

        let claim = service.store().submit_claim(
            "CLM-001",
            "EMP-01",
            "Sandra Schipal",
            Some("AT000000000000000000"),
            "train_ticket",
            4500,
            450,
            chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap_or_default(),
            None,
            None,
            Some("ÖBB Railjet Vienna -> Linz"),
        )?;
        assert_eq!(claim.status, ClaimStatus::Submitted);

        let approved = service.approve_claim(&claim.id)?;
        assert_eq!(approved.status, ClaimStatus::Approved);

        let batch = service.batch_reimburse(
            std::slice::from_ref(&approved.id),
            "Fleet GmbH",
            "AT000000000000000001",
            "BKAUTWWXXXX",
        )?;
        assert_eq!(batch.count, 1);
        assert_eq!(batch.total_amount_cents, 4500);
        assert!(batch.xml_payload.contains("pain.001.001.03"));

        let updated = service.store().get_claim(&claim.id)?;
        assert_eq!(updated.status, ClaimStatus::Reimbursed);

        Ok(())
    }
}
