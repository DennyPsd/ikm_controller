use crate::types::products::Product;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ProductDelete;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ProductDeleteArgs {
  pub id: Uuid,
}

impl IPCMessageDef for ProductDelete {
  type Args = ProductDeleteArgs;
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
