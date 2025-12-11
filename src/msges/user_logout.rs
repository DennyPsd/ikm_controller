use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use taxon_core::prelude::{
  ActionTargetKind, IPCActionKind, IPCMessageDef, IPCMessageDir, IPCTarget,
};

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UserLogout;

impl IPCMessageDef for UserLogout {
  type Args = ();
  type Reply = bool;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Stop)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      user_login: Some("".into()),
      ..ActionTargetKind::User.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
