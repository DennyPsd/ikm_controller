use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Modbus unit id (slave address)
pub type UnitId = u8;

/// Адрес регистра (holding / input)
pub type RegisterAddress = u16;

/// Адрес coil / discrete input
#[allow(dead_code)]
pub type CoilAddress = u16;

/// Какой тип регистра читаем
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModbusRegType {
  Coil,
  DiscreteInput,
  HoldingRegister,
  InputRegister,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModbusValueType {
  // булевое
  Bool,

  // 8-битные
  U8,
  I8,

  // 16-битные
  U16,
  I16,

  // multi-word числа (2+ регистра)
  U32,
  I32,
  F32,
  F64,

  // строки / сырые байты
  AsciiString,
  Utf8String,
  RawBytes,
}

/// Формат слов/байт для multi-word значений (u32/i32/f32/f64 и т.п.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub struct ModbusWordFormat {
  /// Поменять слова местами (word1, word0, word2…)
  pub swap_words: bool,
  /// Поменять байты внутри КАЖДОГО слова (lo, hi)
  pub swap_bytes_in_word: bool,
}

/// Официальные Modbus exception-коды.
/// https://modbus.org/docs/Modbus_Application_Protocol_V1_1b.pdf
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModbusExceptionCode {
  /// 01 – Illegal Function
  IllegalFunction,
  /// 02 – Illegal Data Address
  IllegalDataAddress,
  /// 03 – Illegal Data Value
  IllegalDataValue,
  /// 04 – Slave Device Failure
  SlaveDeviceFailure,
  /// 05 – Acknowledge
  Acknowledge,
  /// 06 – Slave Device Busy
  SlaveDeviceBusy,
  /// 08 – Memory Parity Error
  MemoryParityError,
  /// 0A – Gateway Path Unavailable
  GatewayPathUnavailable,
  /// 0B – Gateway Target Device Failed to Respond
  GatewayTargetFailedToRespond,
  /// Любой другой код, который не знаем
  Unknown(u8),
}

impl ModbusExceptionCode {
  pub fn from_u8(code: u8) -> Self {
    match code {
      0x01 => ModbusExceptionCode::IllegalFunction,
      0x02 => ModbusExceptionCode::IllegalDataAddress,
      0x03 => ModbusExceptionCode::IllegalDataValue,
      0x04 => ModbusExceptionCode::SlaveDeviceFailure,
      0x05 => ModbusExceptionCode::Acknowledge,
      0x06 => ModbusExceptionCode::SlaveDeviceBusy,
      0x08 => ModbusExceptionCode::MemoryParityError,
      0x0A => ModbusExceptionCode::GatewayPathUnavailable,
      0x0B => ModbusExceptionCode::GatewayTargetFailedToRespond,
      other => ModbusExceptionCode::Unknown(other),
    }
  }
  #[allow(dead_code)]
  pub fn as_u8(&self) -> u8 {
    match *self {
      ModbusExceptionCode::IllegalFunction => 0x01,
      ModbusExceptionCode::IllegalDataAddress => 0x02,
      ModbusExceptionCode::IllegalDataValue => 0x03,
      ModbusExceptionCode::SlaveDeviceFailure => 0x04,
      ModbusExceptionCode::Acknowledge => 0x05,
      ModbusExceptionCode::SlaveDeviceBusy => 0x06,
      ModbusExceptionCode::MemoryParityError => 0x08,
      ModbusExceptionCode::GatewayPathUnavailable => 0x0A,
      ModbusExceptionCode::GatewayTargetFailedToRespond => 0x0B,
      ModbusExceptionCode::Unknown(x) => x,
    }
  }
}
