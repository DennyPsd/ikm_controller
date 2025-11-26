// modbus_worker_job.rs
use ractor::{Actor, ActorProcessingErr, ActorRef};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::timeout;
use tokio_serial::SerialStream;
use tracing::info;

use crate::actors::modbus_worker::ModbusWorkerMsg;
use crate::actors::modbus_types::ModbusTimings;

/// Джоб выполняет одно (запись -> чтение) взаимодействие с устройством
#[derive(Debug)]
pub struct ModbusPortJob;

impl ModbusPortJob {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug)]
pub struct ModbusPortJobState {}

#[derive(Debug)]
pub struct ModbusPortJobArgs {
    pub worker: ActorRef<ModbusWorkerMsg>,
    pub cmd: Vec<u8>,
    pub timings: ModbusTimings,
    pub stream: SerialStream,
}

#[derive(Debug)]
pub enum ModbusPortJobMsg {
    Stop,
}

#[ractor::async_trait]
impl Actor for ModbusPortJob {
    type Msg = ModbusPortJobMsg;
    type State = ModbusPortJobState;
    type Arguments = ModbusPortJobArgs;

    async fn pre_start(
        &self,
        myself: ActorRef<Self::Msg>,
        mut args: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        info!("ModbusPortJob: start cmd={:?}", args.cmd);

        let res = send_and_read(
            &mut args.stream,
            &args.cmd,
            args.timings.first_byte_timeout,
            args.timings.per_byte_timeout,
        )
        .await;

        // отправляем обратно результат воркеру
        let _ = args
            .worker
            .cast(ModbusWorkerMsg::ProcessFinished {
                cmd: args.cmd,
                result: res,
                port: args.stream,
            });

        let _ = myself.cast(ModbusPortJobMsg::Stop);
        Ok(ModbusPortJobState {})
    }

    async fn handle(
        &self,
        myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            ModbusPortJobMsg::Stop => {
                let _ = myself.stop(None);
            }
        }
        Ok(())
    }
}

/// Простая функция чтения/записи — записываем и читаем до таймаута.
/// Возвращаем Ok(Vec<u8>) или Err(String)
pub async fn send_and_read(
    port: &mut SerialStream,
    apdu: &[u8],
    first_byte_timeout: Duration,
    per_byte_timeout: Duration,
) -> Result<Vec<u8>, String> {
    // write
    if let Err(e) = port.write_all(apdu).await {
        return Err(format!("write_error: {}", e));
    }
    if let Err(e) = port.flush().await {
        return Err(format!("flush_error: {}", e));
    }

    // short pause to let device respond
    ractor::concurrency::sleep(Duration::from_millis(5)).await;

    // read loop: wait first byte
    let mut response = Vec::new();
    let mut buf = [0u8; 1];

    match timeout(first_byte_timeout, port.read_exact(&mut buf)).await {
        Ok(Ok(_)) => response.push(buf[0]),
        Ok(Err(e)) => return Err(format!("read_first_err: {}", e)),
        Err(_) => return Err("read_first_timeout".into()),
    }

    // read more bytes until per_byte timeout occurs
    loop {
        match timeout(per_byte_timeout, port.read_exact(&mut buf)).await {
            Ok(Ok(_)) => {
                response.push(buf[0]);
                // continue reading; in Modbus we might parse length later
            }
            Ok(Err(e)) => return Err(format!("read_err: {}", e)),
            Err(_) => {
                // per-byte timeout -> assume frame end
                break;
            }
        }
    }

    Ok(response)
}
