use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDocument {
  #[serde(alias = "_id")]
  pub id: Option<String>,
  pub name: String,
  pub hostname: String,
  pub port: String,
}
