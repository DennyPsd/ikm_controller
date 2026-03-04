use crate::msges::event_list::EventListFields;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::components::device::FacilityEventRule;
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};
use uuid::Uuid;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct EventRuleList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
#[allow(dead_code)]
pub enum EventRuleListFields {
  Minimal,
  All,
  Exact(Vec<SmolStr>),
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct EventRuleListArgs {
  pub ids: Vec<Uuid>,
  pub fields: EventListFields,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct EventRuleListReply {
  /// Список
  pub data: Vec<FacilityEventRule>,
}

impl IPCMessageDef for EventRuleList {
  type Args = EventRuleListArgs;
  type Reply = EventRuleListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("FacilityEventRule".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
