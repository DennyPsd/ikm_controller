use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{RegisterAddress, UnitId};
#[allow(dead_code)]
pub const FC06_WRITE_SINGLE_REG: u8 = 0x06;

/// Построить RTU-запрос FC06 (Write Single Register)
#[allow(dead_code)]
pub fn build_fc06_write_single_reg(unit_id: UnitId, addr: RegisterAddress, value: u16) -> Vec<u8> {
  let mut pdu = Vec::with_capacity(4);
  pdu.extend_from_slice(&addr.to_be_bytes());
  pdu.extend_from_slice(&value.to_be_bytes());

  build_rtu_frame(unit_id, FC06_WRITE_SINGLE_REG, &pdu)
}

/// Распарсить ответ FC06, проверить echo
#[allow(dead_code)]
pub fn parse_fc06_write_single_reg(
  unit_id: UnitId,
  expected_addr: RegisterAddress,
  expected_value: u16,
  frame: &[u8],
) -> ModbusResult<()> {
  let (_addr, _func, data) = parse_rtu_response(unit_id, FC06_WRITE_SINGLE_REG, frame)?;

  if data.len() != 4 {
    return Err(ModbusError::InvalidFrame("FC06 response payload len != 4"));
  }

  let addr = u16::from_be_bytes([data[0], data[1]]);
  let value = u16::from_be_bytes([data[2], data[3]]);

  if addr != expected_addr {
    return Err(ModbusError::Decode(format!(
      "FC06 address echo mismatch: expected={}, got={}",
      expected_addr, addr
    )));
  }

  if value != expected_value {
    return Err(ModbusError::Decode(format!(
      "FC06 value echo mismatch: expected={}, got={}",
      expected_value, value
    )));
  }

  Ok(())
}
