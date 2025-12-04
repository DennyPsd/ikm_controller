use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{CoilAddress, UnitId};
#[allow(dead_code)]
pub const FC05_WRITE_SINGLE_COIL: u8 = 0x05;

/// Построить RTU-запрос FC05 (Write Single Coil)
#[allow(dead_code)]
pub fn build_fc05_write_single_coil(unit_id: UnitId, addr: CoilAddress, value: bool) -> Vec<u8> {
  let mut pdu = Vec::with_capacity(4);
  pdu.extend_from_slice(&addr.to_be_bytes());

  let raw: u16 = if value { 0xFF00 } else { 0x0000 };
  pdu.extend_from_slice(&raw.to_be_bytes());

  build_rtu_frame(unit_id, FC05_WRITE_SINGLE_COIL, &pdu)
}

/// Распарсить ответ FC05, проверить echo и вернуть финальное значение coil
#[allow(dead_code)]
pub fn parse_fc05_write_single_coil(
  unit_id: UnitId,
  expected_addr: CoilAddress,
  frame: &[u8],
) -> ModbusResult<bool> {
  let (_addr, _func, data) = parse_rtu_response(unit_id, FC05_WRITE_SINGLE_COIL, frame)?;

  if data.len() != 4 {
    return Err(ModbusError::InvalidFrame("FC05 response payload len != 4"));
  }

  let addr = u16::from_be_bytes([data[0], data[1]]);
  let raw = u16::from_be_bytes([data[2], data[3]]);

  if addr != expected_addr {
    return Err(ModbusError::Decode(format!(
      "FC05 address echo mismatch: expected={}, got={}",
      expected_addr, addr
    )));
  }

  let value = match raw {
    0xFF00 => true,
    0x0000 => false,
    other => {
      return Err(ModbusError::Decode(format!(
        "FC05 unexpected coil value echo: 0x{other:04X}"
      )));
    }
  };

  Ok(value)
}
