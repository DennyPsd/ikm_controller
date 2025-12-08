use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use taxon_core::infrastructure::device::{FacilityEvent, FacilityEventRule};
use taxon_core::infrastructure::user::User;
use taxon_core::prelude::*;
use taxon_core::utils::asyncapi::AsyncapiBuilder;
use uuid::Uuid;

use crate::types::products::Product;
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::Tank;
use crate::types::{kmh::KMHReportInstance, parks::Park};
// use ikm_calc::calculation::kmh::{KMHCalculator, KMHReport};
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
/// Получения списка событий

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct EventList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub enum EventListFields {
  Minimal,
  All,
  Exact(Vec<SmolStr>),
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct EventListArgs {
  pub ids: Vec<Uuid>,
  pub fields: EventListFields,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct EventListReply {
  /// Tanks
  pub data: Vec<FacilityEvent>,
}

impl IPCMessageDef for EventList {
  type Args = EventListArgs;
  type Reply = EventListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("FacilityEvent".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
// ////////////////////////////
/// Получения списка правил событий

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct EventRuleList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
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
  /// Tanks
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
/// Сохранение/изменение правила события

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
/// Получения списка КМХ отчетов

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportList;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub enum KMHReportListFields {
  Minimal, //все кроме дата
  All,
  Exact(Vec<SmolStr>),
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportListArgs {
  pub ids: Vec<Uuid>,
  pub fields: KMHReportListFields,
}

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportListReply {
  /// Tanks
  pub data: Vec<KMHReportInstance>,
}

impl IPCMessageDef for KMHReportList {
  type Args = KMHReportListArgs;
  type Reply = KMHReportListReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("KMHReport".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}
/// Изменение настроек цистерны

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportSet;

impl IPCMessageDef for KMHReportSet {
  /// [KMHReportInstance] - КМХ отчёт
  type Args = KMHReportInstance;
  /// [KMHReportInstance] - КМХ отчёт
  type Reply = KMHReportInstance;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetData)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("KMHReport".into()),
      // data_id: Some(Uuid::max()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}

// ////////////////////////////
/// KMHReportCreate — создать новый КМХ-отчёт (болванка) для устройства

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportCreate;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct KMHReportCreateArgs {
  /// ID танка/резервуара, для которого создаётся отчёт
  pub device_id: Uuid,
}

impl IPCMessageDef for KMHReportCreate {
  type Args = KMHReportCreateArgs;
  type Reply = KMHReportInstance;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }

  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("KMHReport".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }

  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}

// ////////////////////////////
/// KMHReportCalc — пересчитать КМХ-отчёт без сохранения

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct KMHReportCalc;

impl IPCMessageDef for KMHReportCalc {
  /// [KMHReportInstance] - входные данные отчёта
  type Args = KMHReportInstance;
  /// [KMHReportInstance] - пересчитанный отчёт
  type Reply = KMHReportInstance;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    // чистый расчёт, без записи — тоже можно отнести к GetData
    Some(IPCActionKind::GetData)
  }

  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      data_ns: Some("KMHReport".into()),
      ..ActionTargetKind::Data.to_target()
    })
  }

  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
}

#[allow(dead_code)]
pub fn ikm_controller_client_api() -> AsyncapiBuilder {
  AsyncapiBuilder::new(IPCRole::Router)
    .operation::<TankList, ()>()
    .operation::<TankConfigSet, ()>()
    .operation::<EventList, ()>()
    .operation::<EventRuleList, ()>()
    .operation::<EventRuleSet, ()>()
    .operation::<ParkList, ()>()
    .operation::<ProductList, ()>()
    .operation::<KMHReportList, ()>()
    .operation::<KMHReportSet, ()>()
    .operation::<KMHReportCreate, ()>()
    .operation::<KMHReportCalc, ()>()
    .operation::<UserLogin, ()>()
    .operation::<UserLogout, ()>()
}
