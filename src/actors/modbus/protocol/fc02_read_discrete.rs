use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{CoilAddress, UnitId};
use crate::actors::modbus::protocol::utils::unpack_bits;
#[allow(dead_code)]
pub const FC02_READ_DISCRETE: u8 = 0x02;

/// Построить RTU-запрос FC02 (Read Discrete Inputs)
#[allow(dead_code)]
pub fn build_fc02_read_discrete(
  unit_id: UnitId,
  start: CoilAddress,
  quantity: u16,
) -> ModbusResult<Vec<u8>> {
  if quantity == 0 || quantity > 2000 {
    return Err(ModbusError::Decode(
      "quantity for FC02 must be in 1..=2000".into(),
    ));
  }

  let mut pdu = Vec::with_capacity(4);
  pdu.extend_from_slice(&start.to_be_bytes());
  pdu.extend_from_slice(&quantity.to_be_bytes());

  Ok(build_rtu_frame(unit_id, FC02_READ_DISCRETE, &pdu))
}

/// Распарсить RTU-ответ FC02
#[allow(dead_code)]
pub fn parse_fc02_read_discrete(
  unit_id: UnitId,
  quantity: u16,
  frame: &[u8],
) -> ModbusResult<Vec<bool>> {
  let (_addr, _func, data) = parse_rtu_response(unit_id, FC02_READ_DISCRETE, frame)?;

  if data.is_empty() {
    return Err(ModbusError::InvalidFrame("FC02 response has no byte count"));
  }

  let byte_count = data[0] as usize;
  let status_bytes = &data[1..];

  if status_bytes.len() != byte_count {
    return Err(ModbusError::InvalidFrame("FC02 byte count mismatch"));
  }

  unpack_bits(status_bytes, quantity)
}
