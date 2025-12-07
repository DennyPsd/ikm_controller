use crate::actors::modbus::protocol::types::ModbusExceptionCode;
use std::{error::Error, fmt, io};

pub type ModbusResult<T> = Result<T, ModbusError>;

#[derive(Debug, Clone)]
pub enum ModbusError {
  /// Ошибка ввода/вывода (serial, tcp и т.п.)
  Io(String),

  /// Таймаут ожидания ответа
  #[allow(dead_code)]
  Timeout,

  /// CRC не сошёлся
  CrcMismatch { expected: u16, got: u16 },

  /// Жёстко битый/короткий фрейм
  InvalidFrame(&'static str),

  /// Адрес слейва не совпал с ожидаемым
  AddressMismatch { expected: u8, got: u8 },

  /// Функция не совпала с ожидаемой
  FunctionMismatch { expected: u8, got: u8 },

  /// Слейв вернул Modbus exception (func | 0x80)
  Exception {
    function: u8,
    code: ModbusExceptionCode,
  },

  /// Ошибка декодирования полезной нагрузки (формат, длина и т.п.)
  #[allow(dead_code)]
  Decode(String),

  /// Всё остальное
  #[allow(dead_code)]
  Other(String),
}

impl From<io::Error> for ModbusError {
  fn from(e: io::Error) -> Self {
    ModbusError::Io(e.to_string())
  }
}

impl fmt::Display for ModbusError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      ModbusError::Io(msg) => write!(f, "IO error: {msg}"),
      ModbusError::Timeout => write!(f, "Timeout waiting for Modbus response"),
      ModbusError::CrcMismatch { expected, got } => {
        write!(
          f,
          "CRC mismatch: expected=0x{expected:04X}, got=0x{got:04X}"
        )
      }
      ModbusError::InvalidFrame(msg) => write!(f, "Invalid Modbus frame: {msg}"),
      ModbusError::AddressMismatch { expected, got } => write!(
        f,
        "Unit id (address) mismatch: expected={expected}, got={got}"
      ),
      ModbusError::FunctionMismatch { expected, got } => {
        write!(
          f,
          "Function mismatch: expected=0x{expected:02X}, got=0x{got:02X}"
        )
      }
      ModbusError::Exception { function, code } => {
        write!(
          f,
          "Modbus exception (func=0x{function:02X}, code={:?})",
          code
        )
      }
      ModbusError::Decode(msg) => write!(f, "Decode error: {msg}"),
      ModbusError::Other(msg) => write!(f, "{msg}"),
    }
  }
}

impl Error for ModbusError {}
