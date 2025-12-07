use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::types::ModbusExceptionCode;

/// Стандартный Modbus CRC16 (poly=0xA001, init=0xFFFF, LSB first)
pub fn crc16_modbus(data: &[u8]) -> u16 {
  let mut crc: u16 = 0xFFFF;

  for &b in data {
    crc ^= b as u16;
    for _ in 0..8 {
      if (crc & 0x0001) != 0 {
        crc >>= 1;
        crc ^= 0xA001;
      } else {
        crc >>= 1;
      }
    }
  }

  crc
}

/// Собрать RTU-фрейм: [addr][func][payload...][crc_lo][crc_hi]
pub fn build_rtu_frame(addr: u8, func: u8, payload: &[u8]) -> Vec<u8> {
  let mut frame = Vec::with_capacity(2 + payload.len() + 2);
  frame.push(addr);
  frame.push(func);
  frame.extend_from_slice(payload);

  let crc = crc16_modbus(&frame);
  let crc_lo = (crc & 0x00FF) as u8;
  let crc_hi = (crc >> 8) as u8;

  frame.push(crc_lo);
  frame.push(crc_hi);

  frame
}

/// Разобрать RTU-ответ и проверить:
///   * длина >= 4
///   * CRC
///   * unit id (addr)
///   * func == expected_func (или func|0x80 => exception)
///
/// На выход даём кортеж:
///   (addr, func, data_bytes_без_crc)
pub fn parse_rtu_response(
  expected_addr: u8,
  expected_func: u8,
  frame: &[u8],
) -> ModbusResult<(u8, u8, Vec<u8>)> {
  if frame.len() < 4 {
    return Err(ModbusError::InvalidFrame("too short"));
  }

  let len = frame.len();
  let (without_crc, crc_bytes) = frame.split_at(len - 2);
  let crc_from_frame: u16 = (crc_bytes[1] as u16) << 8 | (crc_bytes[0] as u16);
  let crc_calc = crc16_modbus(without_crc);

  if crc_calc != crc_from_frame {
    return Err(ModbusError::CrcMismatch {
      expected: crc_calc,
      got: crc_from_frame,
    });
  }

  let addr = without_crc[0];
  let func = without_crc[1];
  let data = without_crc[2..].to_vec();

  if addr != expected_addr {
    return Err(ModbusError::AddressMismatch {
      expected: expected_addr,
      got: addr,
    });
  }

  // Проверка на exception: func | 0x80
  if (func & 0x80) != 0 {
    // func без старшего бита
    let base_func = func & 0x7F;

    let code_byte = data
      .first()
      .copied()
      .ok_or(ModbusError::InvalidFrame("exception frame has no code"))?;

    let code = ModbusExceptionCode::from_u8(code_byte);

    return Err(ModbusError::Exception {
      function: base_func,
      code,
    });
  }

  if func != expected_func {
    return Err(ModbusError::FunctionMismatch {
      expected: expected_func,
      got: func,
    });
  }

  Ok((addr, func, data))
}
