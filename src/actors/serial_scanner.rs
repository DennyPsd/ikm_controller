use ractor::{Actor, ActorProcessingErr, ActorRef};
use serialport::{SerialPortInfo, SerialPortType};
use smol_str::SmolStr;
use std::{
  collections::{HashMap, HashSet},
  time::Duration,
};
use tokio_serial::SerialPortBuilderExt;
use tracing::{error, info};

use crate::actors::modbus::config::ModbusSettings;
use crate::actors::modbus::modbus_fabric::ModbusFabricMsg;

pub struct SerialScannerState {
  pub fabric: ActorRef<ModbusFabricMsg>,
  pub known: HashMap<SmolStr, SerialPortInfo>, // key = "group:port_key"
  pub settings: ModbusSettings,
}

pub struct SerialScannerActor;

impl SerialScannerActor {
  pub fn new() -> Self {
    Self
  }
}

#[derive(Debug)]
pub enum SerialScannerMsg {
  Tick,
}

/// Сформировать логический id порта по группе и ключу порта:
///   group = "r33", port_key = "port1" -> "r33:port1"
fn make_group_port_id(group: &SmolStr, port_key: &SmolStr) -> SmolStr {
  SmolStr::from(format!("{group}:{port_key}"))
}

/// Обратная операция:
///   "r33:port1" -> Some(("r33".into(), "port1".into()))
///   "r33"       -> None  (нет ':')
pub fn parse_group_port_id(id: &SmolStr) -> Option<(SmolStr, SmolStr)> {
  let s = id.as_str();
  let (group, port_key) = s.split_once(':')?;
  Some((SmolStr::from(group), SmolStr::from(port_key)))
}

#[ractor::async_trait]
impl Actor for SerialScannerActor {
  type Msg = SerialScannerMsg;
  type State = SerialScannerState;
  type Arguments = (ActorRef<ModbusFabricMsg>, ModbusSettings);

  async fn pre_start(
    &self,
    myself: ActorRef<Self::Msg>,
    (fabric, settings): Self::Arguments,
  ) -> Result<Self::State, ActorProcessingErr> {
    let _ = myself.cast(SerialScannerMsg::Tick);
    Ok(SerialScannerState {
      fabric,
      known: HashMap::new(),
      settings,
    })
  }

  async fn handle(
    &self,
    myself: ActorRef<Self::Msg>,
    msg: SerialScannerMsg,
    state: &mut SerialScannerState,
  ) -> Result<(), ActorProcessingErr> {
    match msg {
      SerialScannerMsg::Tick => {
        let ports = match serialport::available_ports() {
          Ok(v) => v,
          Err(e) => {
            error!("SerialScanner: ошибка available_ports(): {e}");
            // сохраняем JoinHandle в переменную, чтобы Clippy не ругался
            let _tick_handle = myself.send_after(Duration::from_secs(1), || SerialScannerMsg::Tick);
            return Ok(());
          }
        };

        // кого увидели в этот проход (логические "group:port")
        let mut seen_ids: HashSet<SmolStr> = HashSet::new();

        for p in ports {
          let full_path = &p.port_name;

          // на macOS пропускаем /dev/cu.*
          if cfg!(target_os = "macos") && full_path.starts_with("/dev/cu.") {
            continue;
          }

          // интересуют только USB-порты
          let SerialPortType::UsbPort(_usb) = &p.port_type else {
            continue;
          };

          // ищем, к какой (group, port_key) привязан этот физический порт
          let found = state
            .settings
            .groups
            .iter()
            .find_map(|(group_name, ports_map)| {
              ports_map.iter().find_map(|(port_key, port_cfg)| {
                if port_cfg.matches_port(full_path) {
                  Some((group_name.clone(), port_key.clone(), &port_cfg.line))
                } else {
                  None
                }
              })
            });

          let (group_name, port_key, line_cfg) = match found {
            Some(t) => t,
            None => {
              info!(
                  phys_port = %full_path,
                  "SerialScanner: для этого порта нет Modbus-конфига (group/port), пропускаем"
              );
              continue;
            }
          };

          // логический идентификатор "r33:port1"
          let id = make_group_port_id(&group_name, &port_key);
          seen_ids.insert(id.clone());

          // уже есть воркер для этого group:port — ничего не делаем
          if state.known.contains_key(&id) {
            continue;
          }

          info!(
              phys_port = %full_path,
              group = %group_name,
              port_key = %port_key,
              id = %id,
              "SerialScanner: найден новый USB-порт для group/port, пробуем открыть"
          );

          // собираем builder из ModbusLineConfig, но с реальным full_path
          let builder = line_cfg.to_tokio_builder(Some(full_path));

          match builder.open_native_async() {
            Ok(stream) => {
              info!(
                  phys_port = %full_path,
                  group = %group_name,
                  port_key = %port_key,
                  id = %id,
                  "SerialScanner: порт открыт, шлём AttachPort в ModbusFabric (id = group:port)"
              );
              let _ = state.fabric.send_message(ModbusFabricMsg::AttachPort {
                port_name: id.clone(), // например "r33:port1"
                stream,
              });
              state.known.insert(id.clone(), p.clone());
            }
            Err(e) => {
              error!(
                  phys_port = %full_path,
                  group = %group_name,
                  port_key = %port_key,
                  id = %id,
                  "SerialScanner: ошибка открытия stream: {e}"
              );
            }
          }
        }

        // отключение: всё, чего не увидели в этом проходе, считаем пропавшим
        let existing_ids: Vec<SmolStr> = state.known.keys().cloned().collect();
        for id in existing_ids {
          if !seen_ids.contains(&id) {
            let _ = state.fabric.cast(ModbusFabricMsg::DetachPort {
              port_name: id.clone(),
            });
            state.known.remove(&id);
            info!(
                id = %id,
                "SerialScanner: логический порт (group:port) пропал, отправили DetachPort"
            );
          }
        }

        let _tick_handle = myself.send_after(Duration::from_secs(1), || SerialScannerMsg::Tick);
      }
    }

    Ok(())
  }
}
