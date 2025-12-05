use crate::actors::mes_back::types::base::CouchModelExt;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParkDocument {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  pub name: String,
}

impl CouchModelExt for ParkDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("park")
  }
}
