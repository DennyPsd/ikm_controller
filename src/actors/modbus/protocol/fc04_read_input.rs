use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{RegisterAddress, UnitId};
#[allow(dead_code)]
pub const FC04_READ_INPUT: u8 = 0x04;

/// Построить RTU-запрос FC04 (Read Input Registers)
#[allow(dead_code)]
pub fn build_fc04_read_input(
  unit_id: UnitId,
  start: RegisterAddress,
  quantity: u16,
) -> ModbusResult<Vec<u8>> {
  if quantity == 0 || quantity > 125 {
    return Err(ModbusError::Decode(
      "quantity for FC04 must be in 1..=125".into(),
    ));
  }

  let mut pdu = Vec::with_capacity(4);
  pdu.extend_from_slice(&start.to_be_bytes());
  pdu.extend_from_slice(&quantity.to_be_bytes());

  Ok(build_rtu_frame(unit_id, FC04_READ_INPUT, &pdu))
}

/// Распарсить RTU-ответ FC04
#[allow(dead_code)]
pub fn parse_fc04_read_input(
  unit_id: UnitId,
  quantity: u16,
  frame: &[u8],
) -> ModbusResult<Vec<u16>> {
  let (_addr, _func, data) = parse_rtu_response(unit_id, FC04_READ_INPUT, frame)?;

  if data.is_empty() {
    return Err(ModbusError::InvalidFrame("FC04 response has no byte count"));
  }

  let byte_count = data[0] as usize;
  let payload = &data[1..];

  if payload.len() != byte_count {
    return Err(ModbusError::InvalidFrame("FC04 byte count mismatch"));
  }

  let quantity = quantity as usize;
  let expected_bytes = quantity * 2;

  if payload.len() != expected_bytes {
    return Err(ModbusError::InvalidFrame(
      "FC04 payload size != quantity * 2",
    ));
  }

  let mut regs = Vec::with_capacity(quantity);
  for chunk in payload.chunks_exact(2) {
    let val = u16::from_be_bytes([chunk[0], chunk[1]]);
    regs.push(val);
  }

  Ok(regs)
}
