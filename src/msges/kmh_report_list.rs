use crate::types::kmh::KMHReportInstance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub enum KMHReportListFields {
  Minimal, //все кроме дата
  All,
  Exact(Vec<SmolStr>),
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportListArgs {
  pub ids: Vec<Uuid>,
  pub fields: KMHReportListFields,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportListReply {
  /// Tanks
  pub data: Vec<KMHReportInstance>,
}

impl IPCMessageDef for KMHReportList {
  type Args = KMHReportListArgs;
  type Reply = KMHReportListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("KMHReport".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
