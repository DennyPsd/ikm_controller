use ikm_calc::calculation::kmh::{KMHCalculator, KMHReport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportInstance {
  pub id: Uuid,
  pub device_id: Uuid,
  pub data: KMHReport,
}
