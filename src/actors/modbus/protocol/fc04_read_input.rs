use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::frame::{build_rtu_frame, parse_rtu_response};
use crate::actors::modbus::protocol::types::{
  ModbusValueType, ModbusWordFormat, RegisterAddress, UnitId,
};
use crate::actors::modbus::protocol::utils::decode_from_regs;

pub const FC04_READ_INPUT: u8 = 0x04;

/// Построить RTU-запрос FC04 (Read Input Registers)
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

  let frame = build_rtu_frame(unit_id, FC04_READ_INPUT, &pdu);

  // debug!(
  //   "FC04 build_fc04_read_input: unit_id={}, start={}, quantity={}, PDU={:02X?}, frame={:02X?}",
  //   unit_id, start, quantity, pdu, frame
  // );

  Ok(frame)
}

/// Низкоуровневый парсер FC04 -> Vec<u16>
pub fn parse_fc04_read_input(
  unit_id: UnitId,
  quantity: u16,
  frame: &[u8],
) -> ModbusResult<Vec<u16>> {
  // debug!(
  //   "FC04 parse_fc04_read_input: raw frame (len={}): {:02X?}",
  //   frame.len(),
  //   frame
  // );

  let (_addr, _func, data) = parse_rtu_response(unit_id, FC04_READ_INPUT, frame)?;

  // debug!(
  //   "FC04 parse_fc04_read_input: after parse_rtu_response: data={:02X?}",
  //   data
  // );

  if data.is_empty() {
    return Err(ModbusError::InvalidFrame("FC04 response has no byte count"));
  }

  let byte_count = data[0] as usize;
  let payload = &data[1..];

  // debug!(
  //   "FC04 parse_fc04_read_input: byte_count={}, payload_len={}, payload={:02X?}",
  //   byte_count,
  //   payload.len(),
  //   payload
  // );

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
  //
  // debug!(
  //   "FC04 parse_fc04_read_input: decoded regs (quantity={}): {:?}",
  //   quantity, regs
  // );

  Ok(regs)
}

/// ВЕРХНЕУРОВНЕВЫЙ парсер FC04 (typed + word_format + scale/offset)
pub fn parse_fc04_read_input_typed(
  unit_id: UnitId,
  quantity: u16,
  frame: &[u8],
  value_type: ModbusValueType,
  word_format: ModbusWordFormat,
  scale: f64,
  offset: f64,
) -> ModbusResult<f64> {
  // debug!(
  //   "FC04 parse_fc04_read_input_typed: unit_id={}, quantity={}, value_type={:?}, \
  //    word_format={{ swap_words: {}, swap_bytes_in_word: {} }}, scale={}, offset={}, raw_frame={:02X?}",
  //   unit_id,
  //   quantity,
  //   value_type,
  //   word_format.swap_words,
  //   word_format.swap_bytes_in_word,
  //   scale,
  //   offset,
  //   frame,
  // );

  let regs = parse_fc04_read_input(unit_id, quantity, frame)?;

  // debug!(
  //   "FC04 parse_fc04_read_input_typed: regs before decode_from_regs: {:?}",
  //   regs
  // );

  let val = decode_from_regs(&regs, value_type, word_format, scale, offset)?;

  // debug!(
  //   "FC04 parse_fc04_read_input_typed: final decoded value (f64) = {}",
  //   val
  // );

  Ok(val)
}
