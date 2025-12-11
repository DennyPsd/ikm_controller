use crate::types::kmh::KMHReportInstance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportSet;

impl IPCMessageDef for KMHReportSet {
  /// [KMHReportInstance] - КМХ отчёт
  type Args = KMHReportInstance;
  /// [KMHReportInstance] - КМХ отчёт
  type Reply = KMHReportInstance;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("KMHReport".into()),
      // data_id: Some(Uuid::max()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
