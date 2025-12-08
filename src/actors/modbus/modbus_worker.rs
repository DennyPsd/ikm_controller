use ractor::{Actor, ActorProcessingErr, ActorRef};
use smol_str::SmolStr;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::SerialStream;
use tracing::{debug, error, info};

use crate::actors::modbus::config::{ModbusPortConfig, ModbusRegisterConfig, ModbusTimings};
use crate::actors::modbus::modbus_fabric::ModbusFabricMsg;
use crate::actors::modbus::protocol::utils::reg_type_to_fc;

use crate::actors::modbus::protocol::fc01_read_coils::{
  build_fc01_read_coils, parse_fc01_read_coils_typed,
};
use crate::actors::modbus::protocol::fc02_read_discrete::{
  build_fc02_read_discrete, parse_fc02_read_discrete_typed,
};
use crate::actors::modbus::protocol::fc03_read_holding::{
  build_fc03_read_holding, parse_fc03_read_holding_typed,
};
use crate::actors::modbus::protocol::fc04_read_input::{
  build_fc04_read_input, parse_fc04_read_input_typed,
};

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
  /// Накопленные результаты за текущий цикл поллинга
  pub pending: Vec<(u16 /*slave*/, u16 /*addr*/, Option<f32>)>,
}

pub struct ModbusWorker;

impl ModbusWorker {
  pub fn new() -> Self {
    Self
  }
}

fn fmt_hex(bytes: &[u8]) -> String {
  let mut s = String::new();
  for (i, b) in bytes.iter().enumerate() {
    if i > 0 {
      s.push(' ');
    }
    use std::fmt::Write as _;
    let _ = write!(&mut s, "{:02X}", b);
  }
  s
}

/// Отправка ГОТОВОГО RTU-кадра и чтение сырых байт.
/// НИЧЕГО не парсим, просто возвращаем полный frame (addr..crc).
pub async fn send_and_read_frame(
  port: &mut SerialStream,
  frame: &[u8],
  first_byte_timeout: Duration,
  _per_byte_timeout: Duration,
) -> Result<Vec<u8>, String> {
  // TX
  debug!(
    "Modbus TX frame ({} bytes): {}",
    frame.len(),
    fmt_hex(frame)
  );

  port
    .write_all(frame)
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

  let frame_rx = buf[..n].to_vec();
  debug!(
    "Modbus RX frame ({} bytes): {}",
    frame_rx.len(),
    fmt_hex(&frame_rx)
  );

  Ok(frame_rx)
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
      pending: Vec::new(),
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

        info!(
          port = %state.port_name,
          poll_index = state.current_index,
          polls_total = state.polls.len(),
          slave = poll.slave_id,
          start_reg = reg.start_reg,
          regs_count = reg.regs_count,
          reg_type = ?reg.reg_type,
          value_type = ?reg.value_type,
          fc,
          "ModbusWorker: начинаем опрос регистра",
        );

        // scale / offset из конфига
        let scale = reg.scale.unwrap_or(1.0);
        let offset = reg.offset.unwrap_or(0.0);

        // --- строим RTU-кадр через протокольные функции ---
        let frame_res: Result<Vec<u8>, String> = match fc {
          1 => {
            // FC01 / Read Coils
            build_fc01_read_coils(poll.slave_id, reg.start_reg, reg.regs_count)
              .map_err(|e| format!("build_fc01_read_coils error: {e:?}"))
          }
          2 => {
            // FC02 / Read Discrete Inputs
            build_fc02_read_discrete(poll.slave_id, reg.start_reg, reg.regs_count)
              .map_err(|e| format!("build_fc02_read_discrete error: {e:?}"))
          }
          3 => {
            // FC03 / Read Holding Registers
            build_fc03_read_holding(poll.slave_id, reg.start_reg, reg.regs_count)
              .map_err(|e| format!("build_fc03_read_holding error: {e:?}"))
          }
          4 => {
            // FC04 / Read Input Registers
            build_fc04_read_input(poll.slave_id, reg.start_reg, reg.regs_count)
              .map_err(|e| format!("build_fc04_read_input error: {e:?}"))
          }
          other => Err(format!("Unsupported function code for read: {other}")),
        };

        let mut value_for_report: Option<f32> = None;

        match frame_res {
          Err(err) => {
            error!(
              port = %state.port_name,
              slave = poll.slave_id,
              addr = reg.start_reg,
              "ModbusWorker: ошибка сборки кадра: {}",
              err,
            );
          }
          Ok(frame) => {
            let res = send_and_read_frame(
              &mut port,
              &frame,
              state.timings.first_byte_timeout,
              state.timings.per_byte_timeout,
            )
            .await;

            match res {
              Err(err) => {
                error!(
                  port = %state.port_name,
                  slave = poll.slave_id,
                  addr = reg.start_reg,
                  "ModbusWorker Poll error: {}",
                  err,
                );
              }
              Ok(frame_rx) => {
                // теперь декодим через parse_fc0X_*_typed
                let decoded: Result<Option<f64>, String> = match fc {
                  1 => {
                    let val = parse_fc01_read_coils_typed(
                      poll.slave_id,
                      reg.regs_count,
                      &frame_rx,
                      scale,
                      offset,
                    )
                    .map_err(|e| format!("parse_fc01_read_coils_typed error: {e:?}"))?;
                    Ok(Some(val))
                  }
                  2 => {
                    let val = parse_fc02_read_discrete_typed(
                      poll.slave_id,
                      reg.regs_count,
                      &frame_rx,
                      scale,
                      offset,
                    )
                    .map_err(|e| format!("parse_fc02_read_discrete_typed error: {e:?}"))?;
                    Ok(Some(val))
                  }
                  3 => {
                    let val = parse_fc03_read_holding_typed(
                      poll.slave_id,
                      reg.regs_count,
                      &frame_rx,
                      reg.value_type,
                      reg.word_format,
                      scale,
                      offset,
                    )
                    .map_err(|e| format!("parse_fc03_read_holding_typed error: {e:?}"))?;
                    Ok(Some(val))
                  }
                  4 => {
                    let val = parse_fc04_read_input_typed(
                      poll.slave_id,
                      reg.regs_count,
                      &frame_rx,
                      reg.value_type,
                      reg.word_format,
                      scale,
                      offset,
                    )
                    .map_err(|e| format!("parse_fc04_read_input_typed error: {e:?}"))?;
                    Ok(Some(val))
                  }
                  other => Err(format!(
                    "Unsupported FC in ModbusWorker decode path: {other}"
                  )),
                };

                match decoded {
                  Ok(Some(v64)) => {
                    value_for_report = Some(v64 as f32);
                  }
                  Ok(None) => {
                    value_for_report = None;
                  }
                  Err(err) => {
                    error!(
                      port = %state.port_name,
                      slave = poll.slave_id,
                      addr = reg.start_reg,
                      "ModbusWorker decode error: {}",
                      err,
                    );
                    value_for_report = None;
                  }
                }
              }
            }
          }
        }

        // Кладём результат этого регистра в pending (даже если None — тоже важно)
        state
          .pending
          .push((poll.slave_id as u16, reg.start_reg, value_for_report));

        // Возвращаем поток
        state.stream = Some(port);

        // Проверяем: был ли это последний регистр цикла?
        let is_last_in_cycle = state.current_index + 1 == state.polls.len();

        if is_last_in_cycle {
          use std::fmt::Write as FmtWrite;

          let mut report = String::new();

          let _ = writeln!(
            &mut report,
            "ModbusWorker: завершён цикл опроса, {} точек",
            state.pending.len()
          );
          let _ = writeln!(&mut report, "+-------+---------+----------------------+");
          let _ = writeln!(&mut report, "| slave |  addr   | value                |");
          let _ = writeln!(&mut report, "+-------+---------+----------------------+");

          for (slave, addr, val_opt) in state.pending.drain(..) {
            match val_opt {
              Some(v) => {
                let _ = writeln!(&mut report, "| {:5} | {:7} | {:>20.6} |", slave, addr, v);
              }
              None => {
                let _ = writeln!(
                  &mut report,
                  "| {:5} | {:7} | {:>20} |",
                  slave, addr, "<нет данных>",
                );
              }
            }

            // Отправляем в Fabric как и раньше
            let _ = state.fabric.send_message(ModbusFabricMsg::WorkerReport {
              port_name: state.port_name.clone(),
              slave,
              addr,
              raw: val_opt,
            });
          }

          let _ = writeln!(&mut report, "+-------+---------+----------------------+");

          info!(port = %state.port_name, "{}", report);
        }

        state.current_index = (state.current_index + 1) % state.polls.len();

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
