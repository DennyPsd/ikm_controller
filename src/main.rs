mod actors;
mod msg_def;
mod msges;
mod types;

use crate::actors::ipc_handler::{IpcHandler, IpcHandlerMsg, IpcHandlerState};
use crate::actors::ipc_kmh_handler::{KmhIpcHandler, KmhIpcHandlerMsg, KmhIpcHandlerState};
use crate::actors::modbus::config::ModbusSettings;
use crate::actors::modbus::modbus_fabric::ModbusFabricActor;
use crate::actors::tank_calc::TankCalcActor;
use taxon_core::components::ipc::IPCRole;
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
      // Создаем пустую конфигурацию Modbus - вся информация будет загружена из Tank
      let modbus_settings = ModbusSettings::default();
      info!("ModBus настройки будут загружены из Tank конфигурации");

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
      let (scanner_actor, _scanner_handle) = module
        .spawn_linked(
          Some("SerialScanner".into()),
          SerialScannerActor::new(),
          (modbus_fabric.clone(), modbus_settings.clone()),
        )
        .await
        .map_err(|err| ModuleError::SpawnErr("SerialScanner".into(), err))?;

      // Передаём ссылку на SerialScanner в ModbusFabric для обновления настроек
      let _ = modbus_fabric.cast(crate::actors::modbus::modbus_fabric::ModbusFabricMsg::SetSerialScanner {
        scanner: scanner_actor.clone(),
      });

      // Загружаем Modbus конфигурацию из Tank (настройки будут переданы в SerialScanner)
      let _ = modbus_fabric.cast(crate::actors::modbus::modbus_fabric::ModbusFabricMsg::LoadTanksModbusConfig);

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
