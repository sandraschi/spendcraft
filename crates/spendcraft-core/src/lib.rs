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

    /// Turns `travel.logged` into a submitted claim.
    ///
    /// Employee, country, and hours or kilometres are required. A missing
    /// country is not treated as Austria, and a missing duration is not
    /// treated as 8 hours. The mileage row is `car` (42 c/km) unless the
    /// event names `car_2026`, `bicycle`, or `motorcycle`. The claim note
    /// includes [`STATUTORY_RATES_AS_OF`].
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
            let outcome = if ev.kind == "travel.logged" {
                self.apply_travel_logged(&ev.id, &ev.payload)
            } else {
                Ok(None)
            };
            match outcome {
                Ok(Some(id)) => {
                    created.push(id);
                    bus.ack("spendcraft", &ev.id)?;
                }
                Ok(None) => bus.ack("spendcraft", &ev.id)?,
                Err(error) => bus.reject("spendcraft", &ev.id, &error.to_string())?,
            }
        }

        Ok(DrainSummary {
            processed,
            created_claims: created,
        })
    }

    fn apply_travel_logged(
        &self,
        event_id: &str,
        payload: &serde_json::Value,
    ) -> Result<Option<String>, SpendCoreError> {
        let employee_id = required_str(payload, "employee_id")?;
        let employee_name = required_str(payload, "employee_name")?;
        let country = required_str(payload, "country")?;
        if !BMF_PER_DIEM_RATES
            .iter()
            .any(|rate| rate.country_code.eq_ignore_ascii_case(country))
        {
            return Err(SpendCoreError::Validation(format!(
                "no per-diem table for {country}"
            )));
        }
        let hours = optional_f64(payload, "hours")?;
        let km = optional_f64(payload, "km")?;
        if hours.is_none() && km.is_none() {
            return Err(SpendCoreError::Validation("hours or km is required".into()));
        }
        let hours = hours.unwrap_or(0.0);
        let km = km.unwrap_or(0.0);
        if hours < 0.0 || km < 0.0 {
            return Err(SpendCoreError::Validation(
                "hours and km must be zero or positive".into(),
            ));
        }
        let vehicle = match optional_str(payload, "vehicle") {
            Some(vehicle) if KNOWN_VEHICLES.contains(&vehicle) => vehicle,
            Some(vehicle) => {
                return Err(SpendCoreError::Validation(format!(
                    "unknown vehicle {vehicle}"
                )));
            }
            None => "car",
        };
        let free_lunch = optional_bool(payload, "free_lunch")?.unwrap_or(false);
        let free_dinner = optional_bool(payload, "free_dinner")?.unwrap_or(false);

        let per_diem =
            SpendStore::calculate_foreign_per_diem(country, hours, false, free_lunch, free_dinner);
        let mileage = SpendStore::calculate_kilometergeld(km, vehicle);
        let total = per_diem + mileage;
        if total <= 0 {
            return Err(SpendCoreError::Validation(
                "travel.logged produced no allowance".into(),
            ));
        }

        let claim_id = format!("EXP-TRV-{event_id}");
        if self.store.get_claim(&claim_id).is_ok() {
            return Ok(None);
        }
        let today = Local::now().date_naive();
        self.store.submit_claim_extended(
            &claim_id,
            employee_id,
            employee_name,
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
                "{STATUTORY_RATES_AS_OF}; {hours}h in {country} ({per_diem}c), {km}km {vehicle} ({mileage}c)"
            )),
        )?;
        Ok(Some(claim_id))
    }
}

const KNOWN_VEHICLES: [&str; 4] = ["car", "car_2026", "bicycle", "motorcycle"];

fn required_str<'a>(payload: &'a serde_json::Value, key: &str) -> Result<&'a str, SpendCoreError> {
    payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| SpendCoreError::Validation(format!("{key} is required")))
}

fn optional_str<'a>(payload: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    payload
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn optional_f64(payload: &serde_json::Value, key: &str) -> Result<Option<f64>, SpendCoreError> {
    match payload.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_f64()
            .filter(|number| number.is_finite())
            .map(Some)
            .ok_or_else(|| SpendCoreError::Validation(format!("{key} must be a number"))),
    }
}

fn optional_bool(payload: &serde_json::Value, key: &str) -> Result<Option<bool>, SpendCoreError> {
    match payload.get(key) {
        None => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .ok_or_else(|| SpendCoreError::Validation(format!("{key} must be a boolean"))),
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

    #[test]
    fn travel_logged_becomes_a_submitted_claim() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!("spendcraft-travel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let bus = Bus::new(&root);
        bus.emit(
            "teamcraft",
            "spendcraft",
            "travel.logged",
            json!({
                "employee_id": "EMP-ADA",
                "employee_name": "Ada",
                "country": "AT",
                "hours": 12.0,
                "km": 10.0
            }),
        )
        .map_err(|e| e.to_string())?;
        bus.emit(
            "teamcraft",
            "spendcraft",
            "travel.logged",
            json!({"employee_id": "EMP-ADA"}),
        )
        .map_err(|e| e.to_string())?;

        let store = SpendStore::open_in_memory().map_err(|e| e.to_string())?;
        let service = SpendService::new(store, Some(bus));
        let summary = service.drain_inbox().map_err(|e| e.to_string())?;
        if summary.processed != 2 || summary.created_claims.len() != 1 {
            return Err(format!(
                "processed {} created {}",
                summary.processed,
                summary.created_claims.len()
            ));
        }
        let claim = service
            .store()
            .get_claim(&summary.created_claims[0])
            .map_err(|e| e.to_string())?;
        if claim.status != ClaimStatus::Submitted || claim.amount_cents != 3060 {
            return Err(format!("claim {:?} {}", claim.status, claim.amount_cents));
        }
        let notes = claim.notes.unwrap_or_default();
        if !notes.contains("ruleset=at-estg-26") {
            return Err(notes);
        }
        let bus = service.bus().ok_or("bus dropped")?;
        let pending = bus.pending("spendcraft").map_err(|e| e.to_string())?;
        if !pending.is_empty() {
            return Err(format!("inbox still has {}", pending.len()));
        }
        let failed = bus
            .history("spendcraft", craft_biz::bus::EventState::Failed, 10)
            .map_err(|e| e.to_string())?;
        if failed.len() != 1 {
            return Err(format!("failed {}", failed.len()));
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn batch_emits_reimbursement_batched() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!("spendcraft-batch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        let bus = Bus::new(&root);
        let store = SpendStore::open_in_memory().map_err(|e| e.to_string())?;
        let service = SpendService::new(store, Some(bus));
        service
            .store()
            .submit_claim(
                "CLM-002",
                "EMP-01",
                "Ada",
                Some("AT611904300234573201"),
                "train_ticket",
                4500,
                450,
                chrono::NaiveDate::from_ymd_opt(2026, 11, 1).unwrap_or_default(),
                None,
                None,
                None,
            )
            .map_err(|e| e.to_string())?;
        service
            .approve_claim("CLM-002")
            .map_err(|e| e.to_string())?;
        let batch = service
            .batch_reimburse(
                &["CLM-002".to_string()],
                "Fleet GmbH",
                "AT000000000000000001",
                "BKAUTWWXXXX",
            )
            .map_err(|e| e.to_string())?;
        if batch.total_amount_cents != 4500 {
            return Err(format!("total {}", batch.total_amount_cents));
        }
        let bus = service.bus().ok_or("bus dropped")?;
        for app in ["buchcraft", "billcraft"] {
            let pending = bus.pending(app).map_err(|e| e.to_string())?;
            let batched = pending
                .iter()
                .find(|event| event.kind == "reimbursement.batched");
            let Some(batched) = batched else {
                return Err(format!("{app} has no reimbursement.batched"));
            };
            let total = batched
                .payload
                .get("total_amount_cents")
                .and_then(serde_json::Value::as_i64);
            if total != Some(4500) {
                return Err(format!("{app} total {total:?}"));
            }
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
}
