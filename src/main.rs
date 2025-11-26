//Для теста отправляем запрос на подписку как subscribeToVars.Json. После чего будем отправлять в ответ данные по всем датчикам из settings.yaml.
// value датчиков рандомно меняются 2с.
mod actors;

use actors::modbus_fabric::{ModbusFabricActor, ModbusFabricMsg};
use actors::serial_scanner::{SerialScannerActor, SerialScannerMsg};
use ractor::Actor;


use clap::Parser;
use smol_str::SmolStr;
use uuid::Uuid;
use taxon_core::utils::logging::{LogLevel, init_logging};
use taxon_core::prelude::{IPCActor, IPCActorArgs, IPCActorMsg, ProcessActor};
use crate::actors::ipc_handler::{IpcHandler, IpcHandlerMsg, IpcHandlerState};
use ractor::{OutputPort};
use taxon_core::prelude::IPCMessageCrate;
use anyhow::anyhow;
use tokio::signal;
use tracing::info;



//Настройки для ws
#[derive(Parser)]
#[command(version = "0.1")]
struct Cli {
  #[arg(short, long, env, default_value = "tcp://127.0.0.1:5556")]
  router_address: SmolStr,
  #[arg(long, action = clap::ArgAction::SetTrue)]
  disable_ui: bool,
  /// Module Identity when its run as dealer
  #[arg(short, long, default_value = "00801ad4-1949-4c46-a883-b1a7812852ff")]
  identity: Uuid,
  #[arg(long, env, default_value = "debug")]
  log_level: Option<LogLevel>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {

    
    //выводим список устройств
    // modbus_fabric.send_message(ModbusFabricMsg::PrintDevices).unwrap();


    //Test 25.11 create IPC-Actor
    let cli = Cli::parse();
    init_logging(cli.log_level.unwrap_or(LogLevel::Debug), !cli.disable_ui);

    let (ipc_router, _ipc_router_handle) = IPCActor
    .spawn(
      Some("router".to_string()),
      IPCActorArgs::default_router()
        .with_address(cli.router_address.clone())
        .with_identity(cli.identity)
        .with_module_name("simple_router"),
      None,
    )
    .await
    .expect("Failed to start IPCActor!");

    let initial_devices = vec![];
    // Создаем актор ModbusFabricActor со списком прочтенных устройств
    let (modbus_fabric, _handle) =
        Actor::spawn(Some("ModbusFabric".into()), ModbusFabricActor::new(initial_devices.clone()), initial_devices.clone()).await?;


  let handler_state = IpcHandlerState {
    hart_fabric: modbus_fabric.clone(),
    ipc_router: ipc_router.clone(),
    subscribers: vec![],
  };

  let (ipc_handler, _ipc_handler_handle) =
    Actor::spawn(Some("IpcHandler".into()), IpcHandler, handler_state).await?;
  
  let output_port: OutputPort<IPCMessageCrate> = OutputPort::default();
  output_port.subscribe(ipc_handler, |msg| Some(IpcHandlerMsg::Ipc(msg)));
  
  let _res = ipc_router
    .send_message(Some(IPCActorMsg::Subscribe(output_port)))
    .map_err(|e| anyhow!("Err to sub {e:?}"));

//-------Test Serial Scanner--------

let (_scanner, _scanner_handle) =
        Actor::spawn(
            Some("SerialScanner".into()),
            SerialScannerActor::new(),
            modbus_fabric.clone()         // события в fabric
        ).await?;

      // первый тик запускается в pre_start, можно вручную
    // serial_scanner.cast(SerialScannerMsg::Tick).ok();



  //Ручной тест для изменения value датчиков, потом уберу
  
    // tokio::spawn({
    //     let serial_scanner = serial_scanner.clone();
    //     let devices = settings.devices.clone();
    //     async move {
    //         loop {

    //             // Эмулируем изменение значений ModBus
    //             for (idx, dev) in devices.iter().enumerate() {
    //                 let new_value = rand::rng().random_range(..10); // случайное значение
    //                 modbus_fabric
    //                     .cast(ModbusFabricMsg::WriteDevice {
    //                         device_idx: idx,
    //                         value: new_value,
    //                     })
    //                     .ok();
    //             }
    //             // Задержка между циклами
    //             tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    //         }
    //     }
    // });

  signal::ctrl_c().await?;
  info!("Shutting down...");

    Ok(())
}
