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
use clap::Parser;
pub use msg_def::*;
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
    _build_shared("ikm_controller", ikm_controller_client_api());
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
        .subscribe(ipc_handler, |msg| Some(IpcHandlerMsg::Ipc(Box::new(msg))))
        .map_err(|err| ModuleError::Run("ClientIpcHandler".into(), err))?;

      // Сканирование портов делает SerialScanner
      Ok(())
    })
  })?
}
fn _build_shared(path: impl Into<PathBuf>, builder: AsyncapiBuilder) -> Result<(), anyhow::Error> {
  let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let schema_path = Path::new(&manifest_dir)
    .join("../shared-types/")
    .join(path.into());
  fs::create_dir_all(schema_path.join("./components"));
  let res = builder.commit2path(schema_path.clone());
  match res {
    Err(err) => {
      println!("Schemas wrote to '{schema_path:?}' error: {err}");
      exit(1);
    }
    Ok(_) => {
      println!("Schemas wrote to '{schema_path:?}'");
    }
  }
  // Validate Schema
  let path_env = std::env::var("Path").unwrap();
  let mut asyncapi = Command::new(if cfg!(target_os = "windows") {
    "asyncapi.cmd"
  } else {
    "asyncapi"
  });
  asyncapi
    .env("Path", path_env.clone())
    .env("PATH", path_env.clone())
    .arg("validate")
    .arg(schema_path.join("asyncapi.yaml"))
    .stdout(Stdio::inherit())
    .stderr(Stdio::inherit())
    .spawn()
    .expect("Schema Validation  error")
    .wait()
    .expect("Schema Validation  error");
  // Create bindings:
  fs::create_dir_all(schema_path.join("./bindings/ts"));
  fs::create_dir_all(schema_path.join("./bindings/csharp"));
  let mut entries = fs::read_dir(schema_path.join("./components"))?
    .map(|res| res.map(|e| e.path()))
    .collect::<Result<Vec<_>, io::Error>>()?;
  entries.sort();
  let jobs: Vec<_> = entries
    .into_iter()
    .map(|entry| {
      let path_env = path_env.clone();
      let schema_path = schema_path.clone();
      thread::spawn(move || {
        let quicktype_cmd = if cfg!(target_os = "windows") {
          "quicktype.cmd"
        } else {
          "quicktype"
        };
        let fname = entry
          .file_stem()
          .unwrap()
          .to_str()
          .unwrap()
          .replace(".schema", "");
        println!("Run 'quicktype.cs' for '{fname}'...");
        Command::new(quicktype_cmd)
          .env("Path", path_env.clone())
          .env("PATH", path_env.clone())
          .arg("--no-combine-classes")
          .arg("-o")
          .arg(
            schema_path
              .join("./bindings/csharp")
              .join(format!("./{fname}.cs")),
          )
          .arg("-s")
          .arg("schema")
          .arg(entry.to_str().unwrap())
          .stdout(Stdio::inherit())
          .stderr(Stdio::inherit())
          .spawn()
          .expect("Schema Validation  error")
          .wait()
          .expect("Schema Validation  error");
        println!("Run 'quicktype_cmd.ts' for '{fname}'...");
        Command::new(quicktype_cmd)
          .env("Path", path_env.clone())
          .env("PATH", path_env.clone())
          .arg("--just-types")
          .arg("--no-combine-classes")
          .arg("--prefer-types")
          .arg("--prefer-unions")
          .arg("--converters")
          .arg("all-objects")
          .arg("-o")
          .arg(
            schema_path
              .join("./bindings/ts")
              .join(format!("./{fname}.ts")),
          )
          .arg("-s")
          .arg("schema")
          .arg(entry.to_str().unwrap())
          .stdout(Stdio::inherit())
          .stderr(Stdio::inherit())
          .spawn()
          .expect("Schema Validation  error")
          .wait()
          .expect("Schema Validation  error");
        ()
      })
    })
    .collect();
  for job in jobs {
    job.join();
  }

  Ok(())
}
