use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use smol_str::SmolStr;
use std::collections::{BTreeMap, HashMap};
use taxon_core::utils::default;
use tokio_serial::SerialStream;
use tracing::{info, warn};
use uuid::Uuid;

use crate::actors::modbus::config::{ModbusPortConfig, ModbusSettings};
use crate::actors::modbus::modbus_worker::{ModbusWorker, ModbusWorkerMsg};
use crate::actors::modbus::protocol::utils::reg_type_to_fc;
use crate::actors::serial_scanner::parse_group_port_id;

use serde_json::json;
use smol_str::SmolStr as SS;
use taxon_core::components::device::{
  FacilityDevice, FacilityDeviceMeta, ModbusDeviceMeta, NAMURStatus,
};

// добавил
use crate::types::parks::Park;
use crate::types::products::Product;

#[derive(Debug)]
pub enum ModbusFabricMsg {
  /// Логическое имя порта, например "r33:port1"
  AttachPort {
    port_name: SmolStr,
    stream: SerialStream,
  },
  DetachPort {
    port_name: SmolStr,
  },

  /// Получить девайсы (по указанным портам, либо по всем)
  #[allow(dead_code)]
  GetDevices {
    port_names: Vec<SmolStr>,
    resp: RpcReplyPort<Vec<FacilityDevice>>,
  },

  /// Получить парки
  #[allow(dead_code)]
  GetParks {
    resp: RpcReplyPort<Vec<Park>>,
  },

  /// Получить продукты
  #[allow(dead_code)]
  GetProducts {
    resp: RpcReplyPort<Vec<Product>>,
  },

  #[allow(dead_code)]
  WriteDevice {
    device_idx: usize,
    value: u16,
  },

  /// Сообщение от воркера: обновление значения
  WorkerReport {
    port_name: SmolStr,
    slave: u16,
    addr: u16,
    raw: Option<f32>,
  },
}

pub struct ModbusFabricActor;

/// Состояние:
/// - один воркер на логический порт (group:port)
/// - по каждому порту — список девайсов
#[derive(Debug)]
pub struct ModbusFabricState {
  pub workers: HashMap<SmolStr, ActorRef<ModbusWorkerMsg>>,
  pub devices: HashMap<SmolStr, Vec<FacilityDevice>>,
  pub settings: ModbusSettings,

  // добавил
  pub parks: Vec<Park>,
  pub products: Vec<Product>,
}

impl ModbusFabricActor {
  pub fn new() -> Self {
    Self
  }
}

#[ractor::async_trait]
impl Actor for ModbusFabricActor {
  type Msg = ModbusFabricMsg;
  type State = ModbusFabricState;
  type Arguments = ModbusSettings;

  async fn pre_start(
    &self,
    _myself: ActorRef<Self::Msg>,
    settings: Self::Arguments,
  ) -> Result<Self::State, ActorProcessingErr> {
    info!("ModBusFabric запущен");
    Ok(ModbusFabricState {
      workers: HashMap::new(),
      devices: HashMap::new(),
      settings,
      parks: Vec::new(),
      products: Vec::new(),
    })
  }

  async fn handle(
    &self,
    myself: ActorRef<Self::Msg>,
    msg: Self::Msg,
    state: &mut Self::State,
  ) -> Result<(), ActorProcessingErr> {
    match msg {
      ModbusFabricMsg::AttachPort { port_name, stream } => {
        info!(port = %port_name, "ModBusFabric: логический порт подключен");

        // port_name вида "r33:port1"
        let (group_id, port_key) = match parse_group_port_id(&port_name) {
          Some(v) => v,
          None => {
            warn!(
                port = %port_name,
                "ModBusFabric: AttachPort: некорректный format port_name, ждал group:port"
            );
            return Ok(());
          }
        };

        // Находим группу
        let group_cfg = match state.settings.groups.get(&group_id) {
          Some(g) => g,
          None => {
            warn!(
                group = %group_id,
                port = %port_name,
                "ModBusFabric: AttachPort: группа не найдена в конфиге"
            );
            return Ok(());
          }
        };

        // Находим конкретный порт внутри группы
        let port_cfg: &ModbusPortConfig = match group_cfg.get(&port_key) {
          Some(pcfg) => pcfg,
          None => {
            warn!(
                group = %group_id,
                port_key = %port_key,
                "ModBusFabric: AttachPort: порт не найден в группе"
            );
            return Ok(());
          }
        };

        // Если уже был воркер на этот порт — гасим
        if let Some(old_worker) = state.workers.remove(&port_name) {
          let _ = old_worker.cast(ModbusWorkerMsg::Stop);
        }
        state.devices.remove(&port_name);

        let timings = port_cfg.timings();

        // Спавним воркер под конкретный логический порт
        let (worker_ref, _jh) = ractor::Actor::spawn(
          Some(format!("modbus_worker:{port_name}")),
          ModbusWorker::new(),
          (
            timings,
            stream,
            myself.clone(),
            port_name.clone(),
            port_cfg.clone(),
          ),
        )
        .await
        .map_err(|e| ActorProcessingErr::from(e.to_string()))?;

        state.workers.insert(port_name.clone(), worker_ref);
        info!(port = %port_name, "ModBusFabric: worker spawned");
        let mut port_devices: Vec<FacilityDevice> = Vec::new();

        for slave_cfg in &port_cfg.slaves {
          for reg_cfg in &slave_cfg.registers {
            let mut attrs = BTreeMap::new();
            attrs.insert(SS::from("value"), json!(0.0));

            let scale = reg_cfg.scale.unwrap_or(1.0);
            let offset = reg_cfg.offset.unwrap_or(0.0);

            // новый конфиг
            attrs.insert(SS::from("scale"), json!(scale));
            attrs.insert(SS::from("offset"), json!(offset));
            // обратная совместимость
            attrs.insert(SS::from("mul"), json!(scale));

            attrs.insert(
              SS::from("value_type"),
              json!(format!("{:?}", reg_cfg.value_type)),
            );

            let _dev_type = format!("{}:{}", slave_cfg.name, reg_cfg.start_reg);

            let device = FacilityDevice {
              device_id: Uuid::now_v7(),
              port_address: port_name.clone(),
              meta: Some(FacilityDeviceMeta::Modbus {
                data: ModbusDeviceMeta {
                  slave: slave_cfg.slave_id as u16,
                  addr: reg_cfg.start_reg,
                  reg: reg_type_to_fc(reg_cfg.reg_type),
                },
              }),
              attrs: Some(attrs),
              ..default()
            };

            port_devices.push(device);
          }
        }
        println!("DEVICES!!!!! {:?}", port_devices);
        state.devices.insert(port_name.clone(), port_devices);
      }

      ModbusFabricMsg::DetachPort { port_name } => {
        info!(port = %port_name, "ModBusFabric: логический порт отключен");
        if let Some(wr) = state.workers.remove(&port_name) {
          let _ = wr.cast(ModbusWorkerMsg::Stop);
        }
        state.devices.remove(&port_name);
      }

      ModbusFabricMsg::GetDevices { port_names, resp } => {
        let mut all_devices: Vec<FacilityDevice> = Vec::new();

        if port_names.is_empty() {
          for devs in state.devices.values() {
            all_devices.extend(devs.clone());
          }
        } else {
          for port in port_names {
            if let Some(devs) = state.devices.get(&port) {
              all_devices.extend(devs.clone());
            }
          }
        }

        let _ = resp.send(all_devices);
      }

      ModbusFabricMsg::GetParks { resp } => {
        let _ = resp.send(state.parks.clone());
      }

      ModbusFabricMsg::GetProducts { resp } => {
        let _ = resp.send(state.products.clone());
      }

      ModbusFabricMsg::WriteDevice { device_idx, value } => {
        let mut index_map: Vec<(SmolStr, usize)> = Vec::new();
        for (port_id, list) in state.devices.iter() {
          for i in 0..list.len() {
            index_map.push((port_id.clone(), i));
          }
        }

        if let Some((port_id, local_idx)) = index_map.get(device_idx).cloned() {
          if let Some(list) = state.devices.get_mut(&port_id)
            && let Some(dev) = list.get_mut(local_idx)
          {
            if let Some(attrs) = dev.attrs.as_mut() {
              attrs.insert(SS::from("value"), json!(value));
            }
            info!(
              "ModbusFabric: device idx {} (port {}, local {}) updated = {}",
              device_idx, port_id, local_idx, value
            );
          }
        } else {
          warn!(
            "ModbusFabric: WriteDevice: global index {} out of range",
            device_idx
          );
        }
      }

      ModbusFabricMsg::WorkerReport {
        port_name,
        slave,
        addr,
        raw,
      } => {
        // info!(port = %port_name, slave, addr, raw = ?raw, "WorkerReport received");
        // Ищем только среди девайсов нужного логического порта
        if let Some(devs_on_port) = state.devices.get_mut(&port_name) {
          for dev in devs_on_port.iter_mut() {
            if let Some(FacilityDeviceMeta::Modbus { data }) = &dev.meta
              && data.slave == slave
              && data.addr == addr
            {
              let raw_value = raw.unwrap_or(0.0) as f64;

              let (scale, offset) = {
                let attrs_ref = dev.attrs.as_ref();
                let scale = attrs_ref
                    .and_then(|m| m.get(&SS::from("scale")))
                    .and_then(|v| v.as_f64())
                    // поддержка старого поля "mul"
                    .or_else(|| {
                      attrs_ref
                          .and_then(|m| m.get(&SS::from("mul")))
                          .and_then(|v| v.as_f64())
                    })
                    .unwrap_or(1.0);

                let offset = attrs_ref
                  .and_then(|m| m.get(&SS::from("offset")))
                  .and_then(|v| v.as_f64())
                  .unwrap_or(0.0);

                (scale, offset)
              };

              let final_value = raw_value * scale + offset;

              //info!("{}", final_value);
              if let Some(attrs) = dev.attrs.as_mut() {
                attrs.insert(SS::from("value"), json!(final_value));
              } else {
                let mut map = BTreeMap::new();
                map.insert(SS::from("value"), json!(final_value));
                dev.attrs = Some(map);
              }

              // Статус девайса. Normal - онлайн, FunctionCheck - офлайн
              if raw.is_some() {
                dev.set_status(NAMURStatus::Normal);
              } else {
                dev.set_status(NAMURStatus::FunctionCheck);
              }
            }
          }
        }
      }
    }

    Ok(())
  }
}
