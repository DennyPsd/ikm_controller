use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::components::device::FacilityEventRule;
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct EventRuleSet;

impl IPCMessageDef for EventRuleSet {
  /// [FacilityEventRule] - Правило события
  type Args = FacilityEventRule;
  /// [FacilityEventRule] - Правило события
  type Reply = FacilityEventRule;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("FacilityEventRule".into()),
      // data_id: Some(Uuid::max()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
