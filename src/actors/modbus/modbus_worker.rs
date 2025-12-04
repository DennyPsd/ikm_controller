use ractor::{Actor, ActorProcessingErr, ActorRef};
use smol_str::SmolStr;
use std::time::Duration;
use tokio_serial::SerialStream;
use tracing::{error, info};

use crate::actors::modbus::config::{ModbusPortConfig, ModbusRegisterConfig, ModbusTimings};
use crate::actors::modbus::modbus_fabric::ModbusFabricMsg;
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::utils::{decode_value, reg_type_to_fc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Debug)]
pub enum ModbusWorkerMsg {
  Poll,
  Stop,
}

#[derive(Debug, Clone)]
pub struct PollItem {
  pub slave_id: u8,
  pub reg: ModbusRegisterConfig,
}

#[derive(Debug)]
pub struct ModbusWorkerState {
  pub stream: Option<SerialStream>,
  pub timings: ModbusTimings,
  pub fabric: ActorRef<ModbusFabricMsg>,
  pub port_name: SmolStr,
  pub polls: Vec<PollItem>,
  pub current_index: usize,
  /// Период опроса ОДНОЙ точки
  pub polling_ms: u64,
}

pub struct ModbusWorker;

impl ModbusWorker {
  pub fn new() -> Self {
    Self
  }
}

#[ractor::async_trait]
impl Actor for ModbusWorker {
  type Msg = ModbusWorkerMsg;
  type State = ModbusWorkerState;
  /// (timings, stream, fabric, port_name("r33:port1"), port_cfg)
  type Arguments = (
    ModbusTimings,
    SerialStream,
    ActorRef<ModbusFabricMsg>,
    SmolStr,
    ModbusPortConfig,
  );

  async fn pre_start(
    &self,
    myself: ActorRef<Self::Msg>,
    (timings, stream, fabric, port_name, port_cfg): Self::Arguments,
  ) -> Result<Self::State, ActorProcessingErr> {
    info!(port = %port_name, "ModbusWorker: запущен (stream активен)");

    // Разворачиваем slaves + их registers в плоский список PollItem
    let mut polls = Vec::<PollItem>::new();
    for slave in &port_cfg.slaves {
      for reg in &slave.registers {
        polls.push(PollItem {
          slave_id: slave.slave_id,
          reg: reg.clone(),
        });
      }
    }

    if polls.is_empty() {
      info!(port = %port_name, "ModbusWorker: нет ни одной точки опроса (polls.is_empty)");
    }

    // планируем первый Poll
    let _poll_handle = myself.send_after(Duration::from_secs(1), || ModbusWorkerMsg::Poll);

    Ok(ModbusWorkerState {
      stream: Some(stream),
      timings,
      fabric,
      port_name,
      polls,
      current_index: 0,
      polling_ms: port_cfg.line.polling_ms,
    })
  }

  async fn handle(
    &self,
    myself: ActorRef<Self::Msg>,
    msg: Self::Msg,
    state: &mut ModbusWorkerState,
  ) -> Result<(), ActorProcessingErr> {
    match msg {
      ModbusWorkerMsg::Poll => {
        if state.stream.is_none() {
          let _poll_handle = myself.send_after(Duration::from_secs(2), || ModbusWorkerMsg::Poll);
          return Ok(());
        }

        if state.polls.is_empty() {
          let _poll_handle = myself.send_after(Duration::from_millis(state.polling_ms), || {
            ModbusWorkerMsg::Poll
          });
          return Ok(());
        }

        let mut port = state.stream.take().unwrap();

        let poll = &state.polls[state.current_index];
        let reg = &poll.reg;
        let fc = reg_type_to_fc(reg.reg_type);

        // payload: [start_hi, start_lo, cnt_hi, cnt_lo]
        let payload = [
          (reg.start_reg >> 8) as u8,
          (reg.start_reg & 0xFF) as u8,
          (reg.regs_count >> 8) as u8,
          (reg.regs_count & 0xFF) as u8,
        ];

        let res = send_and_read(
          &mut port,
          poll.slave_id,
          fc,
          &payload,
          state.timings.first_byte_timeout,
          state.timings.per_byte_timeout,
        )
        .await;

        match res {
          Ok(bytes) => {
            if bytes.is_empty() {
              error!(
                  port = %state.port_name,
                  "ModbusWorker: пустой data-ответ"
              );
              let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                port_name: state.port_name.clone(),
                slave: poll.slave_id as u16,
                addr: reg.start_reg,
                raw: None,
              });
            } else {
              let byte_count = bytes[0] as usize;
              if bytes.len() < 1 + byte_count {
                error!(
                    port = %state.port_name,
                    "ModbusWorker: неконсистентный ответ (byte_count={}): {:02X?}",
                    byte_count,
                    bytes
                );
                let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                  port_name: state.port_name.clone(),
                  slave: poll.slave_id as u16,
                  addr: reg.start_reg,
                  raw: None,
                });
              } else {
                // data-байты регистров
                let data = &bytes[1..1 + byte_count];
                let value_opt = decode_value(reg, data);

                if let Some(val) = value_opt {
                  info!(
                      port = %state.port_name,
                      slave = poll.slave_id,
                      addr = reg.start_reg,
                      "ModbusWorker: значение = {:.4}",
                      val
                  );
                  let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                    port_name: state.port_name.clone(),
                    slave: poll.slave_id as u16,
                    addr: reg.start_reg,
                    raw: Some(val),
                  });
                } else {
                  error!(
                      port = %state.port_name,
                      slave = poll.slave_id,
                      addr = reg.start_reg,
                      "ModbusWorker: decode_value вернул None (value_type={:?}, regs_count={})",
                      reg.value_type,
                      reg.regs_count
                  );
                  let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
                    port_name: state.port_name.clone(),
                    slave: poll.slave_id as u16,
                    addr: reg.start_reg,
                    raw: None,
                  });
                }
              }
            }
          }
          Err(err) => {
            error!(
                port = %state.port_name,
                slave = poll.slave_id,
                addr = reg.start_reg,
                "ModbusWorker Poll error: {}",
                err
            );
            let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
              port_name: state.port_name.clone(),
              slave: poll.slave_id as u16,
              addr: reg.start_reg,
              raw: None,
            });
          }
        }

        // Возвращаем поток
        state.stream = Some(port);

        // Следующая точка
        state.current_index = (state.current_index + 1) % state.polls.len();

        // Планируем следующий Poll
        let _poll_handle = myself.send_after(Duration::from_millis(state.polling_ms), || {
          ModbusWorkerMsg::Poll
        });
      }

      ModbusWorkerMsg::Stop => {
        info!(port = %state.port_name, "ModbusWorker: Stop");
        let _ = myself.stop(None);
      }
    }

    Ok(())
  }
}

/// Отправка RTU-кадра и чтение ответа.
/// Возвращаем **data-часть** Modbus-ответа:
///   для Read Holding/Input Registers:
///     [byte_count, data0, data1, ...]
pub async fn send_and_read(
  port: &mut SerialStream,
  slave: u8,
  func: u8,
  payload: &[u8],
  first_byte_timeout: Duration,
  _per_byte_timeout: Duration, // пока не юзаем, оставляем для будущего
) -> Result<Vec<u8>, String> {
  let frame = build_rtu_frame(slave, func, payload);
  info!("send_and_read: sending {:02X?}", frame);

  port
    .write_all(&frame)
    .await
    .map_err(|e| format!("Write error: {e}"))?;

  let mut buf = [0u8; 256];
  let n = match tokio::time::timeout(first_byte_timeout, port.read(&mut buf)).await {
    Ok(Ok(n)) => n,
    Ok(Err(e)) => return Err(format!("Read error: {e}")),
    Err(_) => return Err("Read timeout".to_string()),
  };

  if n == 0 {
    return Err("No data received".to_string());
  }

  let frame_rx = &buf[..n];
  info!("send_and_read: received {} bytes: {:02X?}", n, frame_rx);

  match parse_rtu_response(slave, func, frame_rx) {
    Ok((_addr, _func, data)) => Ok(data),
    Err(e) => Err(format!("RTU parse error: {e:?}")),
  }
}
