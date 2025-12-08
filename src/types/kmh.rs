use ikm_calc::calculation::kmh::KMHReport;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::infrastructure::facility::SharedData;
use uuid::Uuid;
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportInstance {
  pub id: Uuid,
  pub device_id: Uuid,
  pub data: KMHReport,
}
impl SharedData for KMHReportInstance {
  fn data_id(&self) -> &Uuid {
    &self.id
  }
}
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[allow(dead_code)]
pub struct KMHGraduationTable {
  pub id: Uuid,
  pub data: Vec<Vec<f64>>,
}
impl SharedData for KMHGraduationTable {
  fn data_id(&self) -> &Uuid {
    &self.id
  }
}
