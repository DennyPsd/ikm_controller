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
// /** Загрузка информационную модели датчика */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceList;
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub enum DeviceListFields {
  Minimal,
  All,
  Exact(Vec<SmolStr>),
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceListArgs {
  ids: Vec<Uuid>,
  fields: DeviceListFields,
  rescan: Option<bool>,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceListReply {
  ///Devices
  pub data: Vec<FacilityDevice>,
}

// admin
// engineer  только читает
// operator и читать и писать

// смотреть список
impl IPCMessageDef for DeviceList {
  type Args = DeviceListArgs;
  type Reply = DeviceListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("modbus_controller".into()),
      data_ns: Some("Devices".into()),
      ..ActionTargetKind::Module.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

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

/** Подписка на События статусов устройств */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SubscribeDevicesEvents;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SubscribeDevicesEventsArgs {
  ids: Vec<Uuid>,
}

impl IPCMessageDef for SubscribeDevicesEvents {
  type Args = SubscribeDevicesEventsArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Start)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("modbus_controller".into()),
      event_name: Some("device_events".into()),
      device_id: Some(Uuid::max()),
      ..ActionTargetKind::Subscription.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Send
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

/** Эвент на прочтение всех датчиков группы */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ReadGroupSensors;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ReadGroupSensorsArgs {
  pub group_id: SmolStr, // ID группы из настроек
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ReadGroupSensorsReply {
  pub sensors: BTreeMap<SmolStr, f64>, // name -> value
}

impl IPCMessageDef for ReadGroupSensors {
  type Args = ReadGroupSensorsArgs;
  type Reply = ReadGroupSensorsReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("modbus_controller".into()),
      ..ActionTargetKind::Module.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

/** Вызов calc-функции */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct CallCalcFunction;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct CallCalcFunctionArgs {
  pub group_id: SmolStr,
  pub params: serde_json::Value,
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct CallCalcFunctionReply {
  pub result: serde_json::Value,
}

impl IPCMessageDef for CallCalcFunction {
  type Args = CallCalcFunctionArgs;
  type Reply = CallCalcFunctionReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Command)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("modbus_controller".into()),
      ..ActionTargetKind::Module.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

#[allow(dead_code)]
pub fn modbus_controller_api() -> AsyncapiBuilder {
  AsyncapiBuilder::new(IPCRole::Router)
    .module_name("ModbusController")
    .version("0.1.0")
    .zmq_server("modbus_controller", true)
    .operation::<DeviceList, ()>()
    .operation::<UserLogin, ()>()
    .operation::<UserLogout, ()>()
    .operation::<SubscribeDevicesEvents, ()>()
    .operation::<ReadGroupSensors, ()>()
    .operation::<CallCalcFunction, ()>()
}