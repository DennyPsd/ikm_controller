mod actors;
mod msg_def;
mod msges;
mod types;

use crate::actors::ipc_handler::{IpcHandler, IpcHandlerMsg, IpcHandlerState};
use crate::actors::ipc_kmh_handler::{KmhIpcHandler, KmhIpcHandlerMsg, KmhIpcHandlerState};
use crate::actors::modbus::config::ModbusSettings;
use crate::actors::modbus::modbus_fabric::ModbusFabricActor;
use crate::actors::tank_calc::TankCalcActor;
use actors::serial_scanner::SerialScannerActor;
use clap::Parser;
pub use msg_def::*;
use std::process::exit;
pub use taxon_core::prelude::*;
use tracing::info;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
  /// Name of the person to greet
  #[arg(long)]
  build_shared_types: bool,
}

fn main() -> Result<(), ModuleError> {
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

      // ----- KmhIpcHandler ---------
      let kmh_state = KmhIpcHandlerState {
        ipc_router: ipc_router.clone(),
      };
      let (kmh_handler, _kmh_handle) = module
        .spawn_linked(Some("KmhIpcHandler".into()), KmhIpcHandler, kmh_state)
        .await
        .map_err(|err| ModuleError::SpawnErr("KmhIpcHandler".into(), err))?;
      ipc_router
        .subscribe(kmh_handler.clone(), |msg| {
          Some(KmhIpcHandlerMsg::Ipc(*Box::new(msg)))
        })
        .map_err(|err| ModuleError::Run("KmhIpcHandler".into(), err))?;
      // ----- IpcHandler ---------
      let handler_state = IpcHandlerState {
        hart_fabric: modbus_fabric.clone(),
        ipc_router: ipc_router.clone(),
        subscribers: vec![],
        kmh_handler,
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

      Ok(())
    })
  })?
}
