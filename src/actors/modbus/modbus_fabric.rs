use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use smol_str::SmolStr;
use std::collections::{BTreeMap, HashMap};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use taxon_core::components::data::DataModel;
use taxon_core::utils::default;
use tokio_serial::SerialStream;
use tracing::{info, warn};
use uuid::Uuid;

use crate::actors::modbus::config::{ModbusPortConfig, ModbusSettings};
use crate::actors::modbus::modbus_worker::{ModbusWorker, ModbusWorkerMsg};
use crate::actors::modbus::protocol::utils::reg_type_to_fc;
use crate::actors::serial_scanner::{SerialScannerMsg, parse_group_port_id};
use crate::types::tank_configuration::TankConfig;
use crate::types::tanks::Tank;

use serde_json::json;
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

  /// Установить ссылку на SerialScanner и загрузить конфигурацию из Tank
  SetSerialScanner {
    scanner: SerialScannerRef,
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

/// Ссылка на SerialScanner для передачи обновлённых настроек
pub type SerialScannerRef = ActorRef<SerialScannerMsg>;

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

  // Ссылка на SerialScanner для передачи обновлённых настроек после загрузки из Tank
  pub serial_scanner: Option<SerialScannerRef>,
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
  type Arguments = ();

  async fn pre_start(
    &self,
    _myself: ActorRef<Self::Msg>,
    _args: Self::Arguments,
  ) -> Result<Self::State, ActorProcessingErr> {
    info!("ModBusFabric запущен");
    Ok(ModbusFabricState {
      workers: HashMap::new(),
      devices: HashMap::new(),
      settings: ModbusSettings::default(),
      parks: Vec::new(),
      products: Vec::new(),
      serial_scanner: None,
    })
  }

  async fn handle(
    &self,
    myself: ActorRef<Self::Msg>,
    msg: Self::Msg,
    state: &mut Self::State,
  ) -> Result<(), ActorProcessingErr> {
    match msg {
      // Установка ссылки на SerialScanner и загрузка конфигурации
      ModbusFabricMsg::SetSerialScanner { scanner } => {
        state.serial_scanner = Some(scanner.clone());

        // Загружаем Modbus конфигурацию из Tank
        let tanks = Tank::load_list().await;
        for tank in tanks {
          let tank_id = tank.id;
          let config_path = format!("assets/db/tanks/{}/config.yaml", tank_id);
          let tank_config: Option<TankConfig> = match File::open(&config_path) {
            Ok(mut file) => {
              let mut contents = String::new();
              if file.read_to_string(&mut contents).is_ok() {
                match serde_saphyr::from_str::<TankConfig>(&contents) {
                  Ok(cfg) => Some(cfg),
                  Err(e) => {
                    warn!("Ошибка парсинга config.yaml для танка {}: {}", tank_id, e);
                    None
                  }
                }
              } else {
                None
              }
            }
            Err(e) => {
              warn!(
                "Не удалось открыть config.yaml для танка {}: {}",
                tank_id, e
              );
              None
            }
          };

          if let Some(config) = tank_config {
            if let Some(modbus_cfg) = config.modbus {
              let line_cfg = modbus_cfg.port;
              let slaves: Vec<crate::actors::modbus::config::ModbusSlaveConfig> = modbus_cfg
                .reg_mappings
                .iter()
                .map(
                  |(name, reg_map)| crate::actors::modbus::config::ModbusSlaveConfig {
                    name: name.to_string(),
                    slave_id: reg_map.slave_id,
                    registers: vec![crate::actors::modbus::config::ModbusRegisterConfig {
                      start_reg: reg_map.start_reg,
                      regs_count: reg_map.regs_count,
                      reg_type: reg_map.reg_type.clone(),
                      value_type: reg_map.value_type.clone(),
                      word_format: reg_map.word_format.clone(),
                      scale: reg_map.scale,
                      offset: reg_map.offset,
                      variable: None,
                    }],
                  },
                )
                .collect();

              let port_cfg = crate::actors::modbus::config::ModbusPortConfig {
                line: crate::actors::modbus::config::ModbusLineConfig {
                  port: line_cfg.port.clone(),
                  bitrate: line_cfg.bitrate,
                  parity: line_cfg.parity.clone(),
                  data_bits: line_cfg.data_bits,
                  stop_bits: line_cfg.stop_bits,
                  flow_control: line_cfg.flow_control.clone(),
                  polling_ms: line_cfg.polling_ms,
                  open_timeout_ms: line_cfg.open_timeout_ms,
                  first_byte_timeout_ms: line_cfg.first_byte_timeout_ms,
                  per_byte_timeout_ms: line_cfg.per_byte_timeout_ms,
                  emulation: modbus_cfg.emulation.or(line_cfg.emulation),
                },
                slaves,
                tank_id: Some(tank_id),
                reg_mappings: modbus_cfg.reg_mappings,
              };

              let group_id = SmolStr::from(tank_id.to_string());
              let port_key = SmolStr::from("modbus");

              state
                .settings
                .groups
                .entry(group_id.clone())
                .or_insert_with(HashMap::new)
                .insert(port_key.clone(), port_cfg.clone());

              info!(
                "Добавлена конфигурация порта для танка {}: group={}, port={}",
                tank_id, group_id, port_cfg.line.port
              );
            }
          }
        }

        info!("Загружено групп: {}", state.settings.groups.len());

        // Передаём обновлённые настройки в SerialScanner
        let settings = state.settings.clone();
        let _ = scanner.cast(SerialScannerMsg::UpdateSettings(settings));
      }

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

        // Проверяем режим эмуляции
        let is_emulation = port_cfg.line.emulation.unwrap_or(false);

        if is_emulation {
          // Режим эмуляции: данные читаются из time_series.json через tank_calc
          // Не запускаем ModbusWorker и не создаем устройства
          info!(port = %port_name, "ModBusFabric: режим эмуляции включен, ModbusWorker и устройства не создаются");
        } else {
          // Реальный режим: запускаем ModbusWorker для чтения данных с датчиков
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

          // Создаем устройства только в реальном режиме
          let mut port_devices: Vec<FacilityDevice> = Vec::new();

          for slave_cfg in &port_cfg.slaves {
            for reg_cfg in &slave_cfg.registers {
              let mut attrs = BTreeMap::new();
              attrs.insert("value".to_string(), json!(0.0));

              let scale = reg_cfg.scale.unwrap_or(1.0);
              let offset = reg_cfg.offset.unwrap_or(0.0);

              // новый конфиг
              attrs.insert("scale".to_string(), json!(scale));
              attrs.insert("offset".to_string(), json!(offset));
              // обратная совместимость
              attrs.insert("mul".to_string(), json!(scale));

              attrs.insert(
                "value_type".to_string(),
                json!(format!("{:?}", reg_cfg.value_type)),
              );

              let _dev_type = format!("{}:{}", slave_cfg.name, reg_cfg.start_reg);

              let device = FacilityDevice {
                id: Uuid::now_v7(),
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
              attrs.insert("value".to_string(), json!(value));
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

        // Парсим port_name для получения group_id
        let (group_id, _port_key) = match parse_group_port_id(&port_name) {
          Some(v) => v,
          None => {
            // port_name может быть в другом формате, пропускаем
            return Ok(());
          }
        };

        // Ищем конфигурацию порта для записи в файл
        let mut final_value_for_file: Option<f64> = None;
        let mut var_path_for_file: Option<SmolStr> = None;
        let mut tank_id_for_file: Option<Uuid> = None;

        if let Some(group_cfg) = state.settings.groups.get(&group_id) {
          for (_port_key, port_cfg) in group_cfg.iter() {
            // Проверяем что это режим реальных датчиков
            if port_cfg.line.emulation == Some(true) {
              continue; // Пропускаем эмулированные танки
            }

            // Ищем reg_mappings для данного slave и addr
            for (var_path, reg_map) in &port_cfg.reg_mappings {
              if reg_map.slave_id as u16 == slave && reg_map.start_reg == addr {
                // Нашли соответствие, сохраняем данные для записи
                if let Some(tank_id) = &port_cfg.tank_id {
                  var_path_for_file = Some(var_path.clone());
                  tank_id_for_file = Some(*tank_id);
                  // Прерываем цикл после нахождения первого соответствия
                  break;
                }
              }
            }
            if var_path_for_file.is_some() {
              break;
            }
          }
        }

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
                    .and_then(|m| m.get(&"scale".to_string()))
                    .and_then(|v| v.as_f64())
                    // поддержка старого поля "mul"
                    .or_else(|| {
                      attrs_ref
                          .and_then(|m| m.get(&"mul".to_string()))
                          .and_then(|v| v.as_f64())
                    })
                    .unwrap_or(1.0);

                let offset = attrs_ref
                  .and_then(|m| m.get(&"offset".to_string()))
                  .and_then(|v| v.as_f64())
                  .unwrap_or(0.0);

                (scale, offset)
              };

              let final_value = raw_value * scale + offset;
              final_value_for_file = Some(final_value);

              //info!("{}", final_value);
              if let Some(attrs) = dev.attrs.as_mut() {
                attrs.insert("value".to_string(), json!(final_value));
              } else {
                let mut map = BTreeMap::new();
                map.insert("value".to_string(), json!(final_value));
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

        // Записываем данные в base_vars/ext_vars файлы для танков с emulated=false
        if let (Some(final_value), Some(var_path), Some(tank_id)) =
          (final_value_for_file, var_path_for_file, tank_id_for_file)
        {
          let file_path = if var_path.starts_with("/base_vars/") {
            format!("assets/db/tanks/{}/base_vars.yaml", tank_id)
          } else if var_path.starts_with("/ext_vars/") {
            format!("assets/db/tanks/{}/ext_vars.yaml", tank_id)
          } else {
            warn!(
              "ModbusFabric: неизвестный путь переменной {} для танка {}",
              var_path, tank_id
            );
            return Ok(());
          };

          // Читаем текущий файл
          if let Ok(mut file) = OpenOptions::new().read(true).write(true).open(&file_path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
              // Извлекаем имя переменной из пути
              // Например: "/base_vars/weight" -> "weight"
              let var_name = if var_path.starts_with("/base_vars/") {
                var_path.trim_start_matches("/base_vars/")
              } else {
                var_path.trim_start_matches("/ext_vars/")
              };

              // Формируем новую строку для YAML
              let new_line = format!("{}: {}", var_name, final_value);

              // Простая замена: ищем строку "var_name:" и заменяем её
              let pattern = format!("{}:", var_name);
              if let Some(start_idx) = contents.find(&pattern) {
                // Находим конец строки
                if let Some(end_idx) = contents[start_idx..].find('\n') {
                  let end_pos = start_idx + end_idx;
                  // Заменяем строку
                  contents.replace_range(start_idx..end_pos, &new_line);
                  // Записываем обратно
                  if let Ok(mut file) = OpenOptions::new()
                    .write(true)
                    .truncate(true)
                    .open(&file_path)
                  {
                    if file.write_all(contents.as_bytes()).is_ok() {
                      info!(
                        "ModbusFabric: записано значение {} в {} для танка {}",
                        final_value, var_path, tank_id
                      );
                    }
                  }
                }
              } else {
                // Если переменная не найдена, добавляем её в конец
                contents.push('\n');
                contents.push_str(&new_line);
                if let Ok(mut file) = OpenOptions::new()
                  .write(true)
                  .truncate(true)
                  .open(&file_path)
                {
                  if file.write_all(contents.as_bytes()).is_ok() {
                    info!(
                      "ModbusFabric: добавлено значение {} в {} для танка {}",
                      final_value, var_path, tank_id
                    );
                  }
                }
              }
            }
          } else {
            warn!(
              "ModbusFabric: не удалось открыть файл {} для записи",
              file_path
            );
          }
        }
      }
    }

    Ok(())
  }
}
