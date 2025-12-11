use crate::types::tank_configuration::TankConfig;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankConfigSet;

impl IPCMessageDef for TankConfigSet {
  /// [TankConfig] - Настройки цистерны
  type Args = TankConfig;
  /// [TankConfig] - Настройки цистерны
  type Reply = TankConfig;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("TankConfig".into()),
      device_id: Some(Uuid::max()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
