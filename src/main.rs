mod actors;

use actors::modbus_fabric_actor::{ModbusDevice, ModbusFabricActor, ModbusFabricMsg};
use ractor::Actor;
use serde::Deserialize;
use std::{collections::HashMap, fs};

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






#[derive(Debug, Deserialize)]
pub struct Settings {
    devices: Vec<ModbusDevice>,
}

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
    
    // Читаем файл settings.yaml
    let content = fs::read_to_string("settings.yaml")?;
    let settings: Settings = serde_yaml::from_str(&content)?;
    println!("Loaded devices from settings.yaml: {:?}", settings.devices);

    let mut devices_map = HashMap::new();
    for dev in settings.devices {
        devices_map.insert(dev.id, dev);
    }

    // Создаем актор ModbusFabricActor со списком прочтенных устройств
    let (modbus_fabric, _handle) =
        Actor::spawn(Some("ModbusFabric".into()), ModbusFabricActor::new(HashMap::new()), devices_map).await?;

    // Отправляем сообщение для вывода списка устройств
     modbus_fabric.send_message(ModbusFabricMsg::PrintDevices)?;

    // Записываем значение в устройство с id=1
    modbus_fabric.send_message(ModbusFabricMsg::WriteDevice {
        device_id: 1,
        value: 42,
    }).unwrap();

    // Снова выводим список устройств
    modbus_fabric.send_message(ModbusFabricMsg::PrintDevices).unwrap();
    //modbus_fabric.cast(ModbusFabricMsg::PrintDevices)?;


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
  let handler_state = IpcHandlerState {
    hart_fabric: modbus_fabric.clone(),
    ipc_router: ipc_router.clone(),
  };

  let (ipc_handler, _ipc_handler_handle) =
    Actor::spawn(Some("IpcHandler".into()), IpcHandler, handler_state).await?;
  
  let output_port: OutputPort<IPCMessageCrate> = OutputPort::default();
  output_port.subscribe(ipc_handler, |msg| Some(IpcHandlerMsg::Ipc(msg)));
  
  let _res = ipc_router
    .send_message(Some(IPCActorMsg::Subscribe(output_port)))
    .map_err(|e| anyhow!("Err to sub {e:?}"));
  signal::ctrl_c().await?;
  info!("Shutting down...");

    Ok(())
}
