#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub use spendcraft_store::*;
use chrono::Local;
use craft_biz::bus::Bus;
use craft_biz::BizError;
use serde::{Deserialize, Serialize};
use serde_json::json;
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
            });
            let _ = bus.emit("spendcraft", "billcraft", "expense.approved", payload);
        }

        Ok(claim)
    }

    pub fn reject_claim(&self, id: &str, reason: Option<&str>) -> Result<ExpenseClaim, SpendCoreError> {
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
                let emp_id = ev.payload.get("employee_id").and_then(|v| v.as_str()).unwrap_or("EMP-01");
                let emp_name = ev.payload.get("employee_name").and_then(|v| v.as_str()).unwrap_or("Team Member");
                let hours = ev.payload.get("hours").and_then(|v| v.as_f64()).unwrap_or(8.0);
                let km = ev.payload.get("km").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let free_lunch = ev.payload.get("free_lunch").and_then(|v| v.as_bool()).unwrap_or(false);

                let per_diem = SpendStore::calculate_tagesdiaeten(hours, free_lunch, false);
                let mileage = SpendStore::calculate_kilometergeld(km, "car");
                let total = per_diem + mileage;

                let claim_id = format!("EXP-TRV-{}", Local::now().format("%Y%m%d%H%M%S"));
                let today = Local::now().date_naive();

                if total > 0 && self.store.get_claim(&claim_id).is_err() {
                    let _ = self.store.submit_claim(
                        &claim_id,
                        emp_id,
                        emp_name,
                        None,
                        "travel_per_diem",
                        total,
                        0,
                        today,
                        None,
                        None,
                        Some(&format!("Automated Austrian §26 EStG calculation: {hours}h travel (€{:.2}), {km}km (€{:.2})", per_diem as f64 / 100.0, mileage as f64 / 100.0)),
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
