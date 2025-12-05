use crate::actors::mes_back::types::base::CouchModelExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProductType {
  #[serde(rename = "neft")]
  Neft,
  #[serde(rename = "neftProduct")]
  NeftProduct,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProductMethod {
  #[serde(rename = "manualInput")]
  ManualInput,
  #[serde(rename = "calculation")]
  Calculation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductDocument {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  #[serde(rename = "type")]
  pub product_type: ProductType,

  pub name: String,

  #[serde(rename = "groupId", skip_serializing_if = "Option::is_none")]
  pub group_id: Option<String>,

  pub method: ProductMethod,

  #[serde(rename = "methodInput", skip_serializing_if = "Option::is_none")]
  pub method_input: Option<String>,
}

impl CouchModelExt for ProductDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("product")
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductGroupDocument {
  #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
  pub id: Option<String>,

  #[serde(rename = "_rev", skip_serializing_if = "Option::is_none")]
  pub rev: Option<String>,

  pub name: String,

  #[serde(default)]
  pub products: Vec<String>,
}

impl CouchModelExt for ProductGroupDocument {
  fn document_type(&self) -> Option<&'static str> {
    Some("product_group")
  }
}
