use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{RegisterAddress, UnitId};
#[allow(dead_code)]
pub const FC16_WRITE_MULTIPLE_REGS: u8 = 0x10;

/// Построить RTU-запрос FC16 (Write Multiple Registers)
#[allow(dead_code)]
pub fn build_fc16_write_multiple_regs(
  unit_id: UnitId,
  start: RegisterAddress,
  values: &[u16],
) -> ModbusResult<Vec<u8>> {
  if values.is_empty() {
    return Err(ModbusError::Decode(
      "FC16 requires at least 1 register to write".into(),
    ));
  }

  if values.len() > 123 {
    // стандарт: максимум 123 регистра
    return Err(ModbusError::Decode(
      "FC16 max 123 registers per request".into(),
    ));
  }

  let quantity: u16 = values.len() as u16;

  let mut pdu = Vec::new();
  pdu.extend_from_slice(&start.to_be_bytes());
  pdu.extend_from_slice(&quantity.to_be_bytes());

  let byte_count = values.len() * 2;
  pdu.push(byte_count as u8);

  for v in values {
    pdu.extend_from_slice(&v.to_be_bytes());
  }

  Ok(build_rtu_frame(unit_id, FC16_WRITE_MULTIPLE_REGS, &pdu))
}

/// Распарсить ответ FC16, вернуть подтверждённое количество записанных регистров
#[allow(dead_code)]
pub fn parse_fc16_write_multiple_regs(
  unit_id: UnitId,
  expected_start: RegisterAddress,
  expected_quantity: u16,
  frame: &[u8],
) -> ModbusResult<u16> {
  let (_addr, _func, data) = parse_rtu_response(unit_id, FC16_WRITE_MULTIPLE_REGS, frame)?;

  if data.len() != 4 {
    return Err(ModbusError::InvalidFrame("FC16 response payload len != 4"));
  }

  let start = u16::from_be_bytes([data[0], data[1]]);
  let quantity = u16::from_be_bytes([data[2], data[3]]);

  if start != expected_start {
    return Err(ModbusError::Decode(format!(
      "FC16 start address echo mismatch: expected={}, got={}",
      expected_start, start
    )));
  }

  if quantity != expected_quantity {
    return Err(ModbusError::Decode(format!(
      "FC16 quantity echo mismatch: expected={}, got={}",
      expected_quantity, quantity
    )));
  }

  Ok(quantity)
}
