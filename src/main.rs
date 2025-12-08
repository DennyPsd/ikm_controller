mod actors;
mod msg_def;
mod types;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, exit};
use std::{fs, io, thread};

use actors::serial_scanner::SerialScannerActor;
use taxon_core::utils::asyncapi::AsyncapiBuilder;

use crate::actors::ipc_handler::{IpcHandler, IpcHandlerMsg, IpcHandlerState};
use crate::actors::modbus::config::ModbusSettings;
use crate::actors::modbus::modbus_fabric::ModbusFabricActor;
use crate::actors::tank_calc::TankCalcActor;
use clap::Parser;
pub use msg_def::*;
pub use taxon_core::prelude::*;
use tracing::info;
use uuid::Uuid;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// Name of the person to greet
  #[arg(long)]
  build_shared_types: bool,
}
fn main() -> Result<(), ModuleError> {
  for _ in 0..2 {
    let id = Uuid::now_v7();
    println!("{id}");
  }
  let args = Args::parse();
  if args.build_shared_types {
    let _ = ikm_controller_client_api().commit_bindings("ikm_controller");
    exit(0);
  }
  Module::init(IPCRole::Router).map(|module| {
    module.run(async |_cfg, module, ipc_router, _| {
      // Чтение modbus_settings.yaml
      let modbus_settings: ModbusSettings = {
        let yaml_content = std::fs::read_to_string("modbus_settings.yaml")
          .map_err(|e| ModuleError::Run("read settings".into(), e.into()))?;
        serde_saphyr::from_str(&yaml_content)
          .map_err(|e| ModuleError::Run("parse settings".into(), e.into()))?
      };
      //info!("Loaded modbus settings: {:?}", modbus_settings);
      info!("Настройки ModBus загружены!");

      // ----- ModbusFabric ---------
      let (modbus_fabric, _modbus_fabric_handle) = module
        .spawn_linked(
          Some("ModbusFabric".into()),
          ModbusFabricActor::new(),
          modbus_settings.clone(),
        )
        .await
        .map_err(|err| ModuleError::SpawnErr("ModbusFabric".into(), err))?;

      // ----- SerialScanner ---------
      let (_scanner_actor, _scanner_handle) = module
        .spawn_linked(
          Some("SerialScanner".into()),
          SerialScannerActor::new(),
          (modbus_fabric.clone(), modbus_settings.clone()),
        )
        .await
        .map_err(|err| ModuleError::SpawnErr("SerialScanner".into(), err))?;

      // ----- IpcHandler ---------
      let handler_state = IpcHandlerState {
        hart_fabric: modbus_fabric.clone(),
        ipc_router: ipc_router.clone(),
        subscribers: vec![],
      };
      let (ipc_handler, _ipc_handler_handle) = module
        .spawn_linked(Some("ClientIpcHandler".into()), IpcHandler, handler_state)
        .await
        .map_err(|err| ModuleError::SpawnErr("ClientIpcHandler".into(), err))?;

      ipc_router
        .subscribe(ipc_handler, |msg| Some(IpcHandlerMsg::Ipc(*Box::new(msg))))
        .map_err(|err| ModuleError::Run("ClientIpcHandler".into(), err))?;

      // ----- TankCalcActor ---------
      let (_tank_calc_actor, _tank_calc_handle) = module
        .spawn_linked(Some("TankCalcActor".into()), TankCalcActor::new(), ())
        .await
        .map_err(|err| ModuleError::SpawnErr("TankCalcActor".into(), err))?;

      // Сканирование портов делает SerialScanner
      Ok(())
    })
  })?
}
