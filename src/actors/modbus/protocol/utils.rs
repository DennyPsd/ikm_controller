use crate::actors::modbus::config::ModbusRegisterConfig;
use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::types::{ModbusRegType, ModbusValueType, ModbusWordFormat};
use serde_json::Value as JsonValue;

/// Распаковать биты (coils / discrete inputs) из Modbus-байт.
/// LSB первого байта — первый coil, как в стандарте.
#[allow(dead_code)]
pub fn unpack_bits(bytes: &[u8], quantity: u16) -> ModbusResult<Vec<bool>> {
  let quantity = quantity as usize;
  let mut result = Vec::with_capacity(quantity);

  for i in 0..quantity {
    let byte_idx = i / 8;
    let bit_idx = i % 8;

    if byte_idx >= bytes.len() {
      return Err(ModbusError::InvalidFrame(
        "not enough bytes for coil/discrete status",
      ));
    }

    let bit = (bytes[byte_idx] >> bit_idx) & 0x01;
    result.push(bit != 0);
  }

  Ok(result)
}

/// Упаковать булевы значения в Modbus-формат:
/// LSB первого байта — первый coil.
#[allow(dead_code)]
pub fn pack_bits(bools: &[bool]) -> Vec<u8> {
  if bools.is_empty() {
    return Vec::new();
  }

  let byte_count = bools.len().div_ceil(8);
  let mut bytes = vec![0u8; byte_count];

  for (i, value) in bools.iter().enumerate() {
    if *value {
      let byte_idx = i / 8;
      let bit_idx = i % 8;
      bytes[byte_idx] |= 1 << bit_idx;
    }
  }

  bytes
}

/// Перегоняем регистры в байты с учётом формата слов/байт
#[allow(dead_code)]
pub fn regs_to_bytes(regs: &[u16], fmt: ModbusWordFormat) -> Vec<u8> {
  if regs.is_empty() {
    return Vec::new();
  }

  let mut words: Vec<u16> = regs.to_vec();

  // сначала переставляем слова, если надо
  if fmt.swap_words && words.len() >= 2 {
    words.reverse();
  }

  let mut bytes = Vec::with_capacity(words.len() * 2);

  for w in words {
    let hi = ((w >> 8) & 0xFF) as u8;
    let lo = (w & 0xFF) as u8;

    if fmt.swap_bytes_in_word {
      bytes.push(lo);
      bytes.push(hi);
    } else {
      bytes.push(hi);
      bytes.push(lo);
    }
  }

  bytes
}

/// Универсальный декодер регистров в JSON-значение
#[allow(dead_code)]
pub fn decode_modbus_value(
  regs: &[u16],
  value_type: ModbusValueType,
  fmt: ModbusWordFormat,
  scale: Option<f64>,
  offset: Option<f64>,
) -> ModbusResult<JsonValue> {
  use ModbusValueType::*;

  let s = scale.unwrap_or(1.0);
  let o = offset.unwrap_or(0.0);

  let num = |v: f64| {
    serde_json::Number::from_f64(v)
      .map(JsonValue::Number)
      .ok_or(ModbusError::InvalidFrame("failed to build JSON number"))
  };

  match value_type {
    Bool => {
      let b = regs.first().copied().unwrap_or(0) != 0;
      Ok(JsonValue::Bool(b))
    }
    U16 => {
      let raw = *regs
        .first()
        .ok_or(ModbusError::InvalidFrame("not enough registers for u16"))?;
      num((raw as f64) * s + o)
    }
    I16 => {
      let raw = *regs
        .first()
        .ok_or(ModbusError::InvalidFrame("not enough registers for i16"))? as i16;
      num((raw as f64) * s + o)
    }
    U32 => {
      if regs.len() < 2 {
        return Err(ModbusError::InvalidFrame(
          "not enough registers for u32 (need 2)",
        ));
      }
      let bytes = regs_to_bytes(&regs[0..2], fmt);
      let raw = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
      num((raw as f64) * s + o)
    }
    I32 => {
      if regs.len() < 2 {
        return Err(ModbusError::InvalidFrame(
          "not enough registers for i32 (need 2)",
        ));
      }
      let bytes = regs_to_bytes(&regs[0..2], fmt);
      let raw = i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
      num((raw as f64) * s + o)
    }
    F32 => {
      if regs.len() < 2 {
        return Err(ModbusError::InvalidFrame(
          "not enough registers for f32 (need 2)",
        ));
      }
      let bytes = regs_to_bytes(&regs[0..2], fmt);
      let raw = f32::from_bits(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]));
      num((raw as f64) * s + o)
    }
    F64 => {
      if regs.len() < 4 {
        return Err(ModbusError::InvalidFrame(
          "not enough registers for f64 (need 4)",
        ));
      }
      let bytes = regs_to_bytes(&regs[0..4], fmt);
      let raw = f64::from_bits(u64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
      ]));
      num(raw * s + o)
    }
  }
}

/// Маппер ModbusRegType -> function code
pub fn reg_type_to_fc(rt: ModbusRegType) -> u8 {
  match rt {
    ModbusRegType::Coil => 0x01,            // Read Coils
    ModbusRegType::DiscreteInput => 0x02,   // Read Discrete Inputs
    ModbusRegType::HoldingRegister => 0x03, // Read Holding Registers
    ModbusRegType::InputRegister => 0x04,   // Read Input Registers
  }
}

/// Применяем word_format к байтам: порядок слов/байт
pub fn apply_word_format(bytes: &[u8], fmt: ModbusWordFormat) -> Vec<u8> {
  let mut words: Vec<[u8; 2]> = bytes
    .chunks(2)
    .map(|ch| {
      if ch.len() == 2 {
        [ch[0], ch[1]]
      } else {
        [ch[0], 0]
      }
    })
    .collect();

  // swap_words — разворачиваем порядок 16-битных слов
  if fmt.swap_words {
    words.reverse();
  }

  // swap_bytes_in_word — меняем hi/lo внутри каждого слова
  if fmt.swap_bytes_in_word {
    for w in &mut words {
      w.swap(0, 1);
    }
  }

  let mut out = Vec::with_capacity(bytes.len());
  for w in &words {
    out.push(w[0]);
    out.push(w[1]);
  }
  out
}

/// Превращаем сырые байты регистра в f32 по value_type + word_format
pub fn decode_value(reg: &ModbusRegisterConfig, data_bytes: &[u8]) -> Option<f32> {
  let needed = (reg.regs_count as usize) * 2;
  if data_bytes.len() < needed {
    return None;
  }

  let slice = &data_bytes[..needed];
  let reordered = apply_word_format(slice, reg.word_format);

  use ModbusValueType::*;

  match reg.value_type {
    Bool => {
      // Просто первый байт как флаг
      Some(if reordered[0] != 0 { 1.0 } else { 0.0 })
    }
    U16 => {
      if reordered.len() < 2 {
        return None;
      }
      let v = u16::from_le_bytes([reordered[0], reordered[1]]);
      Some(v as f32)
    }
    I16 => {
      if reordered.len() < 2 {
        return None;
      }
      let v = i16::from_le_bytes([reordered[0], reordered[1]]);
      Some(v as f32)
    }
    U32 => {
      if reordered.len() < 4 {
        return None;
      }
      let v = u32::from_le_bytes([reordered[0], reordered[1], reordered[2], reordered[3]]);
      Some(v as f32)
    }
    I32 => {
      if reordered.len() < 4 {
        return None;
      }
      let v = i32::from_le_bytes([reordered[0], reordered[1], reordered[2], reordered[3]]);
      Some(v as f32)
    }
    F32 => {
      if reordered.len() < 4 {
        return None;
      }
      let v = f32::from_le_bytes([reordered[0], reordered[1], reordered[2], reordered[3]]);
      Some(v)
    }
    F64 => {
      if reordered.len() < 8 {
        return None;
      }
      let v = f64::from_le_bytes([
        reordered[0],
        reordered[1],
        reordered[2],
        reordered[3],
        reordered[4],
        reordered[5],
        reordered[6],
        reordered[7],
      ]);
      Some(v as f32)
    }
  }
}
