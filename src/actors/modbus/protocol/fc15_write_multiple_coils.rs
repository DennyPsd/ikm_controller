use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{CoilAddress, UnitId};
use crate::actors::modbus::protocol::utils::pack_bits;
#[allow(dead_code)]
pub const FC15_WRITE_MULTIPLE_COILS: u8 = 0x0F;

/// Построить RTU-запрос FC15 (Write Multiple Coils)
#[allow(dead_code)]
pub fn build_fc15_write_multiple_coils(
  unit_id: UnitId,
  start: CoilAddress,
  coils: &[bool],
) -> ModbusResult<Vec<u8>> {
  if coils.is_empty() {
    return Err(ModbusError::Decode(
      "FC15 requires at least 1 coil to write".into(),
    ));
  }

  if coils.len() > 1968 {
    // стандарт: максимум 1968 coils = 246 bytes
    return Err(ModbusError::Decode(
      "FC15 max 1968 coils per request".into(),
    ));
  }

  let quantity: u16 = coils.len() as u16;
  let mut pdu = Vec::new();
  pdu.extend_from_slice(&start.to_be_bytes());
  pdu.extend_from_slice(&quantity.to_be_bytes());

  let bytes = pack_bits(coils);
  pdu.push(bytes.len() as u8);
  pdu.extend_from_slice(&bytes);

  Ok(build_rtu_frame(unit_id, FC15_WRITE_MULTIPLE_COILS, &pdu))
}

/// Распарсить ответ FC15, вернуть подтверждённое количество записанных coils
#[allow(dead_code)]
pub fn parse_fc15_write_multiple_coils(
  unit_id: UnitId,
  expected_start: CoilAddress,
  expected_quantity: u16,
  frame: &[u8],
) -> ModbusResult<u16> {
  let (_addr, _func, data) = parse_rtu_response(unit_id, FC15_WRITE_MULTIPLE_COILS, frame)?;

  if data.len() != 4 {
    return Err(ModbusError::InvalidFrame("FC15 response payload len != 4"));
  }

  let start = u16::from_be_bytes([data[0], data[1]]);
  let quantity = u16::from_be_bytes([data[2], data[3]]);

  if start != expected_start {
    return Err(ModbusError::Decode(format!(
      "FC15 start address echo mismatch: expected={}, got={}",
      expected_start, start
    )));
  }

  if quantity != expected_quantity {
    return Err(ModbusError::Decode(format!(
      "FC15 quantity echo mismatch: expected={}, got={}",
      expected_quantity, quantity
    )));
  }

  Ok(quantity)
}
