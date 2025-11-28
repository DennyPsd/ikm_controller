// main.rs Чтение modbus_settings. Запуск акторов, подписка на WS.
// Есть 2 демо-датчика, но без показаний.
mod actors;

use actors::modbus_fabric::{ModbusFabricActor, ModbusFabricMsg};
use actors::modbus_types::ModbusSettings;
use actors::serial_scanner::{SerialScannerActor, SerialScannerMsg};
use ractor::Actor;

use crate::actors::ipc_handler::{IpcHandler, IpcHandlerMsg, IpcHandlerState};
use anyhow::anyhow;
use clap::Parser;
use ractor::OutputPort;
use smol_str::SmolStr;
use taxon_core::prelude::IPCMessageCrate;
use taxon_core::prelude::{IPCActor, IPCActorArgs, IPCActorMsg, IPCRole, ProcessActor};
use taxon_core::utils::logging::{LogLevel, init_logging};
use tokio::signal;
use tracing::info;
use url::Url;
use uuid::Uuid;

use serde_json::json;
use smol_str::SmolStr as SS;
use std::collections::BTreeMap;
use std::path::PathBuf;
use taxon_core::infrastructure::device::{FacilityDevice, FacilityDeviceMeta, ModbusDeviceMeta};

#[derive(Parser)]
#[command(version = "0.1")]
struct Cli {
    #[arg(short, long, env, default_value = "tcp://127.0.0.1:5556")]
    router_address: SmolStr,
    #[arg(long, action = clap::ArgAction::SetTrue)]
    disable_ui: bool,

    #[arg(short, long, default_value = "00801ad4-1949-4c46-a883-b1a7812852ff")]
    identity: Uuid,
    #[arg(long, env, default_value = "debug")]
    log_level: Option<LogLevel>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    init_logging(cli.log_level.unwrap_or(LogLevel::Debug), !cli.disable_ui);

    // parse router address into Url
    let zmq_addr = Url::parse(&cli.router_address[..])
        .map_err(|e| anyhow!("Invalid router address '{}': {}", cli.router_address, e))?;

    // Чтение modbus_settings.yaml
    let modbus_settings: ModbusSettings = {
        let yaml_content = std::fs::read_to_string("modbus_settings.yaml")
            .map_err(|e| anyhow!("Failed to read modbus_settings.yaml: {}", e))?;
        serde_yaml::from_str(&yaml_content)
            .map_err(|e| anyhow!("Failed to parse modbus_settings.yaml: {}", e))?
    };
    info!("Loaded modbus settings: {:?}", modbus_settings);

    let ipc_args = IPCActorArgs {
        module_name: SmolStr::from("simple_router"),
        identity: cli.identity,
        static_path: PathBuf::from("."), // спросить про путь - куда че он
        zmq_router_addr: zmq_addr,
        ws_addr: None,
        http_addr: None,
        role: IPCRole::Router,
    };

    let (ipc_router, _ipc_router_handle) = IPCActor
        .spawn(Some("router".to_string()), ipc_args, None)
        .await
        .expect("Failed to start IPCActor!");

    // Создаём demo устройства для наглядности. Мб удалю. И будем брать из конфига все.
    let mut initial_devices: Vec<FacilityDevice> = Vec::new();
    for i in 0..2 {
        let mut attrs = BTreeMap::new();
        attrs.insert(SS::from("value"), json!(i * 10)); // demo values 0,10,20
        attrs.insert(SS::from("mul"), json!(1.0));
        attrs.insert(SS::from("value_type"), json!("u16"));

        let dev = FacilityDevice {
            device_id: Uuid::new_v4(),
            device_type: "modbus".into(),
            port_address: format!("demo_port_{}", i).into(),
            meta: FacilityDeviceMeta::Modbus {
                data: ModbusDeviceMeta {
                    slave: 1,
                    addr: i as u16,
                    reg: 4,
                },
            },
            connected: true,
            attrs: Some(attrs),
            info: None,
            docs: None,
            events: None,
            active_events: [0; 8],
            diagnostic: None,
        };
        initial_devices.push(dev);
    }

    // Создаем актор ModbusFabricActor со списком прочтенных устройств и настройками
    let (modbus_fabric, _handle) = Actor::spawn(
        Some("ModbusFabric".into()),
        ModbusFabricActor::new(initial_devices.clone()),
        (initial_devices.clone(), modbus_settings.clone()),
    )
    .await?;

    // ---IPC Handler---
    let handler_state = IpcHandlerState {
        hart_fabric: modbus_fabric.clone(),
        ipc_router: ipc_router.clone(),
        subscribers: vec![],
    };

    let (ipc_handler, _ipc_handler_handle) =
        Actor::spawn(Some("IpcHandler".into()), IpcHandler, handler_state).await?;

    // Подписка на сообщения
    let output_port: OutputPort<IPCMessageCrate> = OutputPort::default();
    output_port.subscribe(ipc_handler, |msg| Some(IpcHandlerMsg::Ipc(msg)));

    let _res = ipc_router
        .send_message(Some(IPCActorMsg::Subscribe(output_port)))
        .map_err(|e| anyhow!("Err to sub {e:?}"))?;

    // ---Serial Scanner---
    let (_scanner, _scanner_handle) = Actor::spawn(
        Some("SerialScanner".into()),
        SerialScannerActor::new(),
        (modbus_fabric.clone(), modbus_settings.clone()), // события в fabric и настройки
    )
    .await?;

    signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}
