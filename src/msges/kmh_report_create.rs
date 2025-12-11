use crate::types::kmh::KMHReportInstance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportCreate;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportCreateArgs {
  /// ID танка/резервуара, для которого создаётся отчёт
  pub device_id: Uuid,
}

impl IPCMessageDef for KMHReportCreate {
  type Args = KMHReportCreateArgs;
  type Reply = KMHReportInstance;
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
