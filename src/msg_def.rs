use chrono::{DateTime, Local};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::{BTreeMap, BTreeSet};
use taxon_core::actors::ipc::errors::IPCError;
use taxon_core::infrastructure::device::{FacilityDevice, ModbusDeviceMeta};
use taxon_core::infrastructure::user::User;
use taxon_core::prelude::*;
use taxon_core::utils::asyncapi::AsyncapiBuilder;
use taxon_core::utils::validators::DATE_TIME_RE;
use uuid::Uuid;

use crate::types::tanks::Tank;

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
pub struct TankList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub enum TankListFields {
  Minimal,
  All,
  Exact(Vec<SmolStr>),
}
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct TankListArgs {
  ids: Vec<Uuid>,
  fields: TankListFields,
}
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct DeviceListReply {
  ///Devices
  pub data: Vec<Tank>,
}
impl IPCMessageDef for TankList {
  type Args = TankListArgs;
  type Reply = DeviceListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("Tanks".into()),
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
    .operation::<TankList, ()>()
    .operation::<UserLogin, ()>()
    .operation::<UserLogout, ()>()
}
