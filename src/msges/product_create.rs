use crate::types::products::Product;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ProductCreate;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(tag = "type")]
pub enum ProductCreateArgs {
  Oil {
    title: SmolStr,
    product_weight_net: Option<f64>,
    product_weight_gross: Option<f64>,
    volume_at_15: Option<f64>,
  },
  OilProduct {
    title: SmolStr,
    product_weight: Option<f64>,
    volume_at_15: Option<f64>,
  },
}
impl IPCMessageDef for ProductCreate {
  type Args = ProductCreateArgs;
  type Reply = Product;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetData)
  }

  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("Products".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }

  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
