// src/actors/mes_back/types/devices.rs
use serde::{Deserialize, Serialize};

use crate::actors::mes_back::types::base::CouchModelExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceDocument {
  #[serde(alias = "_id")]
  pub id: Option<String>,

  #[serde(alias = "_rev")]
  pub rev: Option<String>,

  pub name: String,
  pub hostname: String,
  pub port: String,
}

impl CouchModelExt for DeviceDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("device")
  }
}
