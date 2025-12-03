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
/** Загрузка информационную модели датчика */
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
      module_name: Some("ikm_controller".into()),
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
//
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceLoadDD;

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceLoadDDReply {
  /**Базовая модель датчика */
  #[schemars(transform = transform_value)]
  pub data: serde_json::Value,
}

// для просмотра дд
impl IPCMessageDef for DeviceLoadDD {
  type Args = ();
  type Reply = DeviceLoadDDReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Start)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ikm_controller".into()),
      process_name: Some("dd_processing".into()),
      device_id: Some(Uuid::max()),
      ..ActionTargetKind::Process.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceUnloadDD;

impl IPCMessageDef for DeviceUnloadDD {
  type Args = ();
  type Reply = bool;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Stop)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ikm_controller".into()),
      process_name: Some("dd_processing".into()),
      device_id: Some(Uuid::max()),
      ..ActionTargetKind::Process.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}
//
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SendToDevice;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SendToDeviceArgs {
  #[serde(default)]
  pub command: u8,
  #[serde(default)]
  pub data: Option<SmolStr>, // base64 payload (optional)
}

impl IPCMessageDef for SendToDevice {
  type Args = SendToDeviceArgs;
  type Reply = Vec<u8>;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Command)
  }

  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ikm_controller".into()),
      device_id: Some(Uuid::max()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
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
  type Args = SubscribeUIDArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Start)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ikm_controller".into()),
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
// /////// METHODS /////// //
/** Загрузка информационную модели датчика */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct RegisterModule;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct RegisterModuleArgs {}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct RegisterModuleReply {
  /** Message encoding: 0 = CBOR, 1 = JSON, 2 = XML */
  pub prefer_msg_encoding: u8,
}
impl Default for RegisterModuleReply {
  fn default() -> Self {
    Self {
      prefer_msg_encoding: 1,
    }
  }
}

impl IPCMessageDef for RegisterModule {
  type Args = RegisterModuleArgs;
  type Reply = RegisterModuleReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Register)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
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
/** Загрузка информационную модели датчика */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct LoadDD;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct LoadDDArgs {
  pub device_id: Uuid,
  pub controller: SmolStr, // короткое имя контроллера/порта
  pub meta: ModbusDeviceMeta,
}

impl IPCMessageDef for LoadDD {
  type Args = LoadDDArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Start)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Send
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}
/** Загрузка информационную модели датчика */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UnloadDD;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UnloadDDArgs {
  /// com_rel_id
  pub device_id: Uuid,
}

impl IPCMessageDef for UnloadDD {
  type Args = UnloadDDArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Stop)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Send
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}
/** Получение первичной клиентской информационной модели для формирования меню:
  - Получение списка переменных
  - Получение коллекций, массивов
  - Получение меню
  - Получение методов
  - Получение картинок
  - Получение графиков, чартов, форм
  - Получение остальных элементов
*/
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ClientModelBuilded;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct ClientModelBuildedArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub model: serde_json::Value,
}

impl IPCMessageDef for ClientModelBuilded {
  type Args = ClientModelBuildedArgs;
  type Reply = ();
  type ErrorArgs = ();
  fn event() -> Option<IPCEventKind> {
    Some(IPCEventKind::DataChanged)
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

/** Подписка на изменения значения UID атрибута меню клиентской информационной модели */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SubscribeUID;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SubscribeUIDArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub node_paths: BTreeSet<SmolStr>,
}

impl IPCMessageDef for SubscribeUID {
  type Args = SubscribeUIDArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Start)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      event_name: Some("UIDChanged".into()),
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

/** Отписка на изменения значения UID атрибута меню клиентской информационной модели */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UnsubscribeUID;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UnsubscribeUIDArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub node_paths: BTreeSet<SmolStr>,
}

impl IPCMessageDef for UnsubscribeUID {
  type Args = UnsubscribeUIDArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Stop)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      event_name: Some("UIDChanged".into()),
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

/** Оповещение об изменениях значения UID атрибута мен */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UIDChanged;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UIDChangedArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub node_path: SmolStr,
  pub data: SmolStr,
}

impl IPCMessageDef for UIDChanged {
  type Args = UIDChangedArgs;
  type Reply = ();
  type ErrorArgs = IPCError;

  fn event() -> Option<IPCEventKind> {
    Some(IPCEventKind::DataChanged)
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

/** Подписка на изменения значения параметров клиентской информационной модели */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SubscribeParams;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct SubscribeParamsArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub node_paths: BTreeSet<SmolStr>,
}

impl IPCMessageDef for SubscribeParams {
  type Args = SubscribeParamsArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Start)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      event_name: Some("ParamChanged".into()),
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

/** Отписка на изменения значения параметров клиентской информационной модели*/
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UnsubscribeParams;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct UnsubscribeParamsArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub node_paths: BTreeSet<SmolStr>,
}

impl IPCMessageDef for UnsubscribeParams {
  type Args = UnsubscribeParamsArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Stop)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      event_name: Some("ParamChanged".into()),
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

/** Оповещение об изменениях значения параметра */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ParamChanged;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "PascalCase")]
pub struct ParamChangedArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub node_path: SmolStr,
  pub value: serde_json::Value,
}

impl IPCMessageDef for ParamChanged {
  type Args = ParamChangedArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn event() -> Option<IPCEventKind> {
    Some(IPCEventKind::VarsChanged)
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

/** Запись параметров, изменённых в UI cache */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ChangeParams;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ChangeParamsArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub trx_id: Uuid,
  /// `Map<param_node_path,Value>`
  pub change: BTreeMap<SmolStr, serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct ChangeParamsReply {
  /// `Map<param_node_path,save_or_not>`
  pub changed: BTreeMap<SmolStr, bool>,
}

impl IPCMessageDef for ChangeParams {
  type Args = ChangeParamsArgs;
  type Reply = ChangeParamsReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::SetVars)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Send
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

/** Применение значений параметров (commit)*/
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct CommitParams;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
#[skip_serializing_none]
pub struct CommitParamsArgs {
  ///com_rel_id
  pub device_id: Uuid,
  pub trx_id: Uuid,
  /// `Map<param_node_path,Value>`
  pub params: BTreeSet<SmolStr>,
  pub force: Option<bool>,
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct CommitParamsReply {
  pub committed: BTreeSet<SmolStr>,
  pub failed: BTreeMap<SmolStr, SmolStr>,
}
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct CommitParamsErr {
  pub device_id: Uuid,
  pub param_id: SmolStr,
  pub code: i32,
  pub message: SmolStr,
  // #[schemars(pattern(*DATE_TIME_RE))]
  // pub occurred_at: Option<DateTime<Local>>,
}
impl IPCMessageDef for CommitParams {
  type Args = CommitParamsArgs;
  type Reply = CommitParamsReply;
  type ErrorArgs = CommitParamsErr;

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Command)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      ..ActionTargetKind::Device.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Send
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

/** Оповещение об ошибке при записи, если датчик вернул статус ошибки. */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceLinkLost;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
#[skip_serializing_none]
pub struct DeviceLinkLostArgs {
  pub device_id: Uuid,
  pub reason: Option<SmolStr>,

  #[schemars(pattern(*DATE_TIME_RE))]
  pub last_seen: Option<DateTime<Local>>,
  pub timeout_ms: Option<u64>,
}

impl IPCMessageDef for DeviceLinkLost {
  type Args = DeviceLinkLostArgs;
  type Reply = ();
  type ErrorArgs = ();

  fn event() -> Option<IPCEventKind> {
    Some(IPCEventKind::CriticalHappen)
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Receive
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

// // Сервис запуска методов

// #[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
// pub struct RunMethod;

// #[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
// pub struct RunMethodArgs {
//     pub id: Uuid,
//     pub method: SmolStr,
//     pub params: serde_json::Value,
// }

// #[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
// pub struct RunMethodReply {
//     pub result: serde_json::Value,
// }

// impl IPCMessageDef for RunMethod {
//     type Args = RunMethodArgs;
//     type Reply = RunMethodReply;
//     type Error = IPCError;

//     fn action() -> Option<IPCActionKind> {
//         Some(IPCActionKind::Command)
//     }
//     fn target() -> Option<IPCTarget> {
//         Some(IPCTarget {
//             module_name: Some("ddngine".into()),
//             ..ActionTargetKind::Device.to_target()
//         })
//     }
//     fn direction() -> IPCMessageDir {
//         IPCMessageDir::Receive
//     }
// }

// // Сервис вызова UI builtin методов

// #[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
// pub struct CallUIBuiltin;

// #[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
// pub struct CallUIBuiltinArgs {
//     pub name: SmolStr,
//     #[skip_serializing_none]
//     pub id: Option<Uuid>,
//     pub payload: serde_json::Value,
// }

// #[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
// pub struct CallUIBuiltinReply {
//     pub result: serde_json::Value,
// }

// impl IPCMessageDef for CallUIBuiltin {
//     type Args = CallUIBuiltinArgs;
//     type Reply = CallUIBuiltinReply;
//     type Error = IPCError;

//     fn action() -> Option<IPCActionKind> {
//         Some(IPCActionKind::Command)
//     }
//     fn target() -> Option<IPCTarget> {
//         Some(IPCTarget {
//             module_name: Some("ddngine".into()),
//             ..ActionTargetKind::Module.to_target()
//         })
//     }
//     fn direction() -> IPCMessageDir {
//         IPCMessageDir::Receive
//     }
// }

/** Блокировка DDngin от изменений в DD информационной модели всех кроме клиента, который выполняет блокировку. */
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct StartTx;

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
#[skip_serializing_none]
pub struct StartTxArgs {
  pub device_id: Uuid,
  pub timeout_ms: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
#[skip_serializing_none]
pub struct StartTxReply {
  pub trx_id: Uuid,
  #[schemars(pattern(*DATE_TIME_RE))]
  pub expires_at: Option<DateTime<Local>>,
}

impl IPCMessageDef for StartTx {
  type Args = StartTxArgs;
  type Reply = StartTxReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::Command)
  }
  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ddngine".into()),
      ..ActionTargetKind::Module.to_target()
    })
  }
  fn direction() -> IPCMessageDir {
    IPCMessageDir::Send
  }
  fn acl() -> Option<Vec<SmolStr>> {
    Some(vec!["engineer".into(), "operator".into()])
  }
}

#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceHistory;

/// Аргументы: просто UUID датчика
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceHistoryArgs {
  pub device_id: Uuid,
}

/// Одна запись истории параметра
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceHistoryEntry {
  #[schemars(pattern(*DATE_TIME_RE))]
  pub date: DateTime<Local>,
  pub param: SmolStr,
  pub value: serde_json::Value,
  pub new_value: serde_json::Value,
}

/// Ответ: история по датчику
#[derive(Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq, Eq, Hash)]
pub struct DeviceHistoryReply {
  /// com_rel_id / device_id
  pub device_id: Uuid,
  /// map: param_id -> список изменений по времени
  pub history: BTreeMap<SmolStr, Vec<DeviceHistoryEntry>>,
}

impl IPCMessageDef for DeviceHistory {
  type Args = DeviceHistoryArgs;
  type Reply = DeviceHistoryReply;
  type ErrorArgs = ();

  fn action() -> Option<IPCActionKind> {
    Some(IPCActionKind::GetData)
  }

  fn target() -> Option<IPCTarget> {
    Some(IPCTarget {
      module_name: Some("ikm_controller".into()),
      data_ns: Some("DeviceHistory".into()),
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
pub fn ikm_controller_api() -> AsyncapiBuilder {
  AsyncapiBuilder::new(IPCRole::Router)
    .module_name("IkmController")
    .version("0.1.1")
    .zmq_server("ikm_controller", true)
    .zmq_server("ddngine", false)
    .operation::<RegisterModule, ()>()
    .operation::<LoadDD, ()>()
    .operation::<UnloadDD, ()>()
    .operation::<ClientModelBuilded, ()>()
    .operation::<SubscribeUID, ()>()
    .operation::<UnsubscribeUID, ()>()
    .operation::<UIDChanged, ()>()
    .operation::<SubscribeParams, ()>()
    .operation::<UnsubscribeParams, ()>()
    .operation::<ParamChanged, ()>()
    .operation::<ChangeParams, ()>()
    .operation::<CommitParams, ()>()
}
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
