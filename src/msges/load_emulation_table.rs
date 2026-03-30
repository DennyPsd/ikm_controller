use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};
use uuid::Uuid;

/// LoadGradTable — загрузить/обновить градуировочную таблицу для устройства

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct LoadEmulationTable;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct LoadEmulationTableArgs {
  /// ID танка/резервуара, для которого загружается таблица
  pub device_id: Uuid,

  /// Градуировочная таблица в base64 (например, CSV, закодированный в base64)
  pub table: String,
}

impl IPCMessageDef for LoadEmulationTable {
  type Args = LoadEmulationTableArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetData)
  }

  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("Tank".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }

  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
