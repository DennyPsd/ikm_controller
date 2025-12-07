use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::infrastructure::user::User;
use taxon_core::prelude::*;
use taxon_core::utils::asyncapi::AsyncapiBuilder;
use uuid::Uuid;

use crate::types::parks::Park;
use crate::types::products::Product;
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::Tank;

// ////////////////////////////
/// Авторизация
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
/// Получения списка цистерн

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
  pub ids: Vec<Uuid>,
  pub fields: TankListFields,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct TankListReply {
  /// Tanks
  pub data: Vec<Tank>,
}

impl IPCMessageDef for TankList {
  type Args = TankListArgs;
  type Reply = TankListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
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
/// Изменение настроек цистерны

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
// ////////////////////////////
/// ParkList

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ParkList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub enum ParkListFields {
  Minimal,
  All,
  Exact(Vec<SmolStr>),
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ParkListArgs {
  pub ids: Vec<String>,
  pub fields: ParkListFields,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ParkListReply {
  pub data: Vec<Park>,
}

impl IPCMessageDef for ParkList {
  type Args = ParkListArgs;
  type Reply = ParkListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("Parks".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}

// ////////////////////////////
/// ProductList

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ProductList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub enum ProductListFields {
  Minimal,
  All,
  Exact(Vec<SmolStr>),
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ProductListArgs {
  pub ids: Vec<Uuid>,
  pub fields: ProductListFields,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ProductListReply {
  pub data: Vec<Product>,
}

impl IPCMessageDef for ProductList {
  type Args = ProductListArgs;
  type Reply = ProductListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("Products".into()),
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
    .operation::<TankConfigSet, ()>()
    .operation::<ParkList, ()>()
    .operation::<ProductList, ()>()
    .operation::<UserLogin, ()>()
    .operation::<UserLogout, ()>()
}
