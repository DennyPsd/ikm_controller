use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::infrastructure::device::{FacilityEventRule, FacilitySeverity};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct EventRuleCreate;
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
#[serde(tag = "type")]
pub enum EventRuleCreateArgs {
  /// var.name,min,max
  LimitsExceeded {
    tank_id: Option<Uuid>,
    var_path: Vec<SmolStr>,
    min: Option<i64>,
    max: Option<i64>,
    description: SmolStr,
    help: SmolStr,
    name: SmolStr,
    severity: FacilitySeverity,
    threshold: Option<f32>,
  },
}
impl TryInto<FacilityEventRule> for EventRuleCreateArgs {
  type Error = ();
  fn try_into(self) -> Result<FacilityEventRule, Self::Error> {
    let EventRuleCreateArgs::LimitsExceeded {
      tank_id,
      var_path,
      min,
      max,
      description,
      help,
      name,
      severity,
      threshold,
    } = self;
    Ok(FacilityEventRule::LimitsExceeded {
      id: Uuid::now_v7(),
      target: IPCTarget {
        data_ns: Some("Tank".into()),
        data_id: tank_id,
        ..ActionTargetKind::Data.to_target()
      },
      target_title: Some("Tank".into()),
      var_path,
      min,
      max,
      description,
      help,
      name,
      severity,
      threshold,
      can_end: false,
    })
  }
}
impl IPCMessageDef for EventRuleCreate {
  /// [FacilityEventRule] - Правило события
  type Args = EventRuleCreateArgs;
  /// [FacilityEventRule] - Правило события
  type Reply = FacilityEventRule;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("FacilityEventRule".into()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
