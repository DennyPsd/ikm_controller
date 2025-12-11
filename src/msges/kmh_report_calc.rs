use crate::types::kmh::KMHReportInstance;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportCalc;

impl IPCMessageDef for KMHReportCalc {
  /// [KMHReportInstance] - входные данные отчёта
  type Args = KMHReportInstance;
  /// [KMHReportInstance] - пересчитанный отчёт
  type Reply = KMHReportInstance;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    // чистый расчёт, без записи — тоже можно отнести к GetData
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
