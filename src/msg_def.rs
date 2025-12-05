use chrono::{DateTime, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::*;
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet};
use taxon_core::actors::ipc::errors::IPCError;
use taxon_core::infrastructure::device::{FacilityDevice, ModbusDeviceMeta};
use taxon_core::infrastructure::user::User;
use taxon_core::prelude::*;
use taxon_core::utils::asyncapi::AsyncapiBuilder;
use taxon_core::utils::validators::DATE_TIME_RE;
use uuid::Uuid;

// ////////////////////////////
/** Авторизация */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UserLogin;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UserLoginArgs {
  pub login: String,
  pub password: String,
}

impl IPCMessageDef for UserLogin {
  type Args = UserLoginArgs;
  type Reply = User;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Start)
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
// ////////////////////////////
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct GetTanksPrimaryVars;

impl IPCMessageDef for GetTanksPrimaryVars {
  type Args = ();
  type Reply = bool;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Stop)
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
// ////////////////////////////
#[allow(dead_code)]
pub fn ikm_controller_client_api() -> AsyncapiBuilder {
  AsyncapiBuilder::new(IPCRole::Router)
    .module_name("IkmController")
    .version("0.1.1")
    .zmq_server("ikm_controller", true)
    .zmq_server("ddngine", false)
    .operation::<DeviceList, ()>()
    .operation::<DeviceLoadDD, ()>()
    .operation::<DeviceUnloadDD, ()>()
    .operation::<SendToDevice, ()>()
    .operation::<UserLogin, ()>()
    .operation::<UserLogout, ()>()
    .operation::<DeviceHistory, ()>()
}
