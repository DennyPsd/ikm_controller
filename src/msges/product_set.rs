use crate::types::products::Product;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ProductSet;

impl IPCMessageDef for ProductSet {
  type Args = Product;
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
