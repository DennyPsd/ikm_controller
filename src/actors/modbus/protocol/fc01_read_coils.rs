use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{CoilAddress, UnitId};
use crate::actors::modbus::protocol::utils::unpack_bits;

pub const FC01_READ_COILS: u8 = 0x01;

/// Построить RTU-запрос FC01 (Read Coils)
pub fn build_fc01_read_coils(
  unit_id: UnitId,
  start: CoilAddress,
  quantity: u16,
) -> ModbusResult<Vec<u8>> {
  if quantity == 0 || quantity > 2000 {
    return Err(ModbusError::Decode(
      "quantity for FC01 must be in 1..=2000".into(),
    ));
  }

  let mut pdu = Vec::with_capacity(4);
  pdu.extend_from_slice(&start.to_be_bytes());
  pdu.extend_from_slice(&quantity.to_be_bytes());

  Ok(build_rtu_frame(unit_id, FC01_READ_COILS, &pdu))
}

/// Низкоуровневый парсер FC01 -> Vec<bool>
pub fn parse_fc01_read_coils(
  unit_id: UnitId,
  quantity: u16,
  frame: &[u8],
) -> ModbusResult<Vec<bool>> {
  let (_addr, _func, data) = parse_rtu_response(unit_id, FC01_READ_COILS, frame)?;

  if data.is_empty() {
    return Err(ModbusError::InvalidFrame("FC01 response has no byte count"));
  }

  let byte_count = data[0] as usize;
  let status_bytes = &data[1..];

  if status_bytes.len() != byte_count {
    return Err(ModbusError::InvalidFrame("FC01 byte count mismatch"));
  }

  unpack_bits(status_bytes, quantity)
}

/// ВЕРХНЕУРОВНЕВЫЙ парсер FC01:
/// берём первый бит как 0/1, применяем scale/offset, отдаём f64
pub fn parse_fc01_read_coils_typed(
  unit_id: UnitId,
  quantity: u16,
  frame: &[u8],
  scale: f64,
  offset: f64,
) -> ModbusResult<f64> {
  let bits = parse_fc01_read_coils(unit_id, quantity, frame)?;
  let first = bits.first().copied().unwrap_or(false);
  let raw = if first { 1.0_f64 } else { 0.0_f64 };
  Ok(raw * scale + offset)
}
