use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::time::Duration;

use tokio_serial::{self, DataBits, FlowControl, Parity, StopBits};

use crate::actors::modbus::protocol::types::{
  ModbusRegType, ModbusValueType, ModbusWordFormat, RegisterAddress, UnitId,
};

// port
// register_mapping поле

#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ModbusTimings {
  pub first_byte_timeout: Duration,
  pub per_byte_timeout: Duration,
  #[allow(dead_code)]
  pub max_preamble_ff: usize,
}

/// Настройки одной физической линии (конкретный /dev/ttyUSB0 / COMx)
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ModbusPortConfig {
  /// Имя порта: "/dev/ttyUSB0", "COM3" и т.п.
  pub port: String,

  /// Скорость
  pub bitrate: u32,

  /// "none" | "even" | "odd"
  #[serde(default = "default_parity")]
  pub parity: String,

  /// 5..8
  #[serde(default = "default_data_bits")]
  pub data_bits: u8,

  /// 1 или 2
  #[serde(default = "default_stop_bits")]
  pub stop_bits: u8,

  /// "none" | "hardware" | "software"
  /// (синонимы: "rts_cts" -> hardware, "xon_xoff" -> software)
  #[serde(default)]
  pub flow_control: Option<String>,

  /// Период опроса одного сенсора (мс)
  #[serde(default = "default_polling_ms")]
  pub polling_ms: u64,

  /// Таймаут открытия порта (builder.timeout)
  #[serde(default = "default_open_timeout_ms")]
  pub open_timeout_ms: u64,

  /// Modbus-тайминги
  #[serde(default = "default_first_byte_timeout_ms")]
  pub first_byte_timeout_ms: u64,

  #[serde(default = "default_per_byte_timeout_ms")]
  pub per_byte_timeout_ms: u64,
}

/// Конфиг **одного регистра** (одной точки измерения)
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ModbusRegisterMapping {
  /// Modbus unit id
  pub slave_id: UnitId,
  /// Адрес регистра (holding/input)
  pub start_reg: RegisterAddress,

  /// сколько регистров читать (1/2/4)
  pub regs_count: u16,

  /// что именно читаем (FC 01/02/03/04 и т.п.)
  pub reg_type: ModbusRegType,

  /// как интерпретировать прочитанные регистры (u16, i16, f32, i32 и т.п.)
  pub value_type: ModbusValueType,

  /// порядок слов/байт (ABCD / BADC / CDAB / DCBA и т.п.)
  #[serde(default)]
  pub word_format: ModbusWordFormat,

  /// (raw * scale) + offset
  #[serde(default)]
  pub scale: Option<f64>,

  #[serde(default)]
  pub offset: Option<f64>,
}

/// Один **порт внутри группы** (port1, port2, ...)
#[derive(Default, Deserialize, Serialize, Debug, Clone, JsonSchema, PartialEq)]
pub struct ModbusConfig {
  /// Настройки линии
  #[serde(flatten)]
  pub port: ModbusPortConfig,

  /// Мапинг переменных танка на регистры Modbus
  /// @example  {"/base_vars/weight": {...}}
  pub reg_mappings: HashMap<SmolStr, ModbusRegisterMapping>,

  /// Режим эмуляции true - читает данные из time_series.json, false - из реальных датчиков
  #[serde(default)]
  pub emulation: Option<bool>,
}

impl ModbusPortConfig {
  /// Превращаем поля из YAML в ModbusTimings
  pub fn timings(&self) -> ModbusTimings {
    ModbusTimings {
      first_byte_timeout: Duration::from_millis(self.first_byte_timeout_ms),
      per_byte_timeout: Duration::from_millis(self.per_byte_timeout_ms),
      max_preamble_ff: 0,
    }
  }

  /// Собираем настроенный tokio-serial builder.
  ///
  /// `override_port` — если хотим использовать фактический путь (из `serialport::available_ports`),
  /// а не то, что прописано в `port`.
  pub fn to_tokio_builder(&self, override_port: Option<&str>) -> tokio_serial::SerialPortBuilder {
    let port_name = override_port.unwrap_or(&self.port);

    let mut builder = tokio_serial::new(port_name, self.bitrate)
      .timeout(Duration::from_millis(self.open_timeout_ms));

    // parity
    let parity = match self.parity.as_str() {
      "none" => Parity::None,
      "even" => Parity::Even,
      "odd" => Parity::Odd,
      _ => Parity::None,
    };
    builder = builder.parity(parity);

    // data bits
    let dbits = match self.data_bits {
      5 => DataBits::Five,
      6 => DataBits::Six,
      7 => DataBits::Seven,
      8 => DataBits::Eight,
      _ => DataBits::Eight,
    };
    builder = builder.data_bits(dbits);

    // stop bits
    let sbits = match self.stop_bits {
      1 => StopBits::One,
      2 => StopBits::Two,
      _ => StopBits::One,
    };
    builder = builder.stop_bits(sbits);

    // flow control
    let fc = self.flow_control.as_deref().unwrap_or("none");
    let fc_mode = match fc {
      "hardware" | "rts_cts" => FlowControl::Hardware,
      "software" | "xon_xoff" => FlowControl::Software,
      _ => FlowControl::None,
    };
    builder.flow_control(fc_mode)
  }
}

impl ModbusConfig {
  pub fn timings(&self) -> ModbusTimings {
    self.port.timings()
  }

  pub fn matches_port(&self, port: &str) -> bool {
    self.port.port == port
  }
}

fn default_parity() -> String {
  "none".into()
}
fn default_data_bits() -> u8 {
  8
}
fn default_stop_bits() -> u8 {
  1
}
fn default_polling_ms() -> u64 {
  1000
}
fn default_open_timeout_ms() -> u64 {
  1500
}
fn default_first_byte_timeout_ms() -> u64 {
  200
}
fn default_per_byte_timeout_ms() -> u64 {
  50
}
