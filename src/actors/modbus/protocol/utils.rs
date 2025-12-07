use crate::actors::modbus::protocol::error::{ModbusError, ModbusResult};
use crate::actors::modbus::protocol::types::{ModbusRegType, ModbusValueType, ModbusWordFormat};

/// Распаковать биты (coils / discrete inputs) из Modbus-байт.
/// LSB первого байта — первый coil, как в стандарте.
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

/// Маппер ModbusRegType -> function code
pub fn reg_type_to_fc(rt: ModbusRegType) -> u8 {
  match rt {
    ModbusRegType::Coil => 0x01,            // Read Coils
    ModbusRegType::DiscreteInput => 0x02,   // Read Discrete Inputs
    ModbusRegType::HoldingRegister => 0x03, // Read Holding Registers
    ModbusRegType::InputRegister => 0x04,   // Read Input Registers
  }
}

/// Применяем настройки word_format к набору регистров:
///   * swap_bytes_in_word — меняем байты внутри каждого слова
///   * swap_words — меняем местами слова парами: (w0,w1) -> (w1,w0), (w2,w3)->(w3,w2), ...
pub fn apply_word_format(regs: &[u16], fmt: ModbusWordFormat) -> Vec<u16> {
  // сначала свопаем байты внутри слова (если нужно)
  let mut words: Vec<u16> = regs
    .iter()
    .map(|w| {
      if fmt.swap_bytes_in_word {
        let [hi, lo] = w.to_be_bytes();
        u16::from_be_bytes([lo, hi])
      } else {
        *w
      }
    })
    .collect();

  // потом свопаем слова парами (если нужно)
  if fmt.swap_words {
    let mut swapped = Vec::with_capacity(words.len());
    let mut iter = words.chunks_exact(2);
    for pair in &mut iter {
      swapped.push(pair[1]);
      swapped.push(pair[0]);
    }
    // если нечётное число слов — хвост добавляем как есть
    let rem = iter.remainder();
    if !rem.is_empty() {
      swapped.push(rem[0]);
    }
    words = swapped;
  }

  words
}

/// Универсальный декодер "сырые регистры -> одно ЧИСЛО f64".
/// Строки/Raw здесь НЕ обрабатываем — для них должен быть отдельный декодер.
pub fn decode_from_regs(
  regs: &[u16],
  value_type: ModbusValueType,
  word_format: ModbusWordFormat,
  scale: f64,
  offset: f64,
) -> ModbusResult<f64> {
  use ModbusValueType::*;

  if regs.is_empty() {
    return Err(ModbusError::Decode("no registers in response".into()));
  }

  let raw: f64 = match value_type {
    // --- BOOL ---
    Bool => {
      // Любой ненулевой первый регистр — true
      let b = regs[0] != 0;
      if b { 1.0 } else { 0.0 }
    }

    // --- 8-битные ---
    // Семантика:
    //   * берём ПЕРВЫЙ регистр
    //   * если swap_bytes_in_word = true — меняем в нём байты
    //   * ВСЕГДА берём младший байт
    U8 | I8 => {
      let mut w = regs[0];

      if word_format.swap_bytes_in_word {
        let [hi, lo] = w.to_be_bytes();
        w = u16::from_be_bytes([lo, hi]);
      }

      let byte = (w & 0x00FF) as u8;

      match value_type {
        U8 => byte as f64,
        I8 => (byte as i8) as f64,
        _ => unreachable!(),
      }
    }

    // --- 16-битные ---
    // Также учитываем только swap_bytes_in_word для первого регистра.
    U16 | I16 => {
      let mut w = regs[0];

      if word_format.swap_bytes_in_word {
        let [hi, lo] = w.to_be_bytes();
        w = u16::from_be_bytes([lo, hi]);
      }

      match value_type {
        U16 => w as u64 as f64,
        I16 => (w as i16) as f64,
        _ => unreachable!(),
      }
    }

    // --- multi-word: сначала применяем полный word_format (байты + слова) ---
    U32 | I32 | F32 | F64 => {
      let words = apply_word_format(regs, word_format);

      match value_type {
        U32 => {
          if words.len() < 2 {
            return Err(ModbusError::Decode(
              "U32 requires at least 2 registers".into(),
            ));
          }
          let bytes = [
            (words[0] >> 8) as u8,
            (words[0] & 0xFF) as u8,
            (words[1] >> 8) as u8,
            (words[1] & 0xFF) as u8,
          ];
          let v = u32::from_be_bytes(bytes);
          v as f64
        }

        I32 => {
          if words.len() < 2 {
            return Err(ModbusError::Decode(
              "I32 requires at least 2 registers".into(),
            ));
          }
          let bytes = [
            (words[0] >> 8) as u8,
            (words[0] & 0xFF) as u8,
            (words[1] >> 8) as u8,
            (words[1] & 0xFF) as u8,
          ];
          let v = i32::from_be_bytes(bytes);
          v as f64
        }

        F32 => {
          if words.len() < 2 {
            return Err(ModbusError::Decode(
              "F32 requires at least 2 registers".into(),
            ));
          }
          let bytes = [
            (words[0] >> 8) as u8,
            (words[0] & 0xFF) as u8,
            (words[1] >> 8) as u8,
            (words[1] & 0xFF) as u8,
          ];
          let v = f32::from_be_bytes(bytes);
          v as f64
        }

        F64 => {
          if words.len() < 4 {
            return Err(ModbusError::Decode(
              "F64 requires at least 4 registers".into(),
            ));
          }
          let bytes = [
            (words[0] >> 8) as u8,
            (words[0] & 0xFF) as u8,
            (words[1] >> 8) as u8,
            (words[1] & 0xFF) as u8,
            (words[2] >> 8) as u8,
            (words[2] & 0xFF) as u8,
            (words[3] >> 8) as u8,
            (words[3] & 0xFF) as u8,
          ];
          f64::from_be_bytes(bytes)
        }

        _ => unreachable!(),
      }
    }

    // Строки и сырые байты — тут НЕ декодируем, пусть другой слой это делает.
    AsciiString | Utf8String | RawBytes => {
      return Err(ModbusError::Decode(
        "decode_from_regs (numeric) called for non-numeric value_type".into(),
      ));
    }
  };

  // Масштабирование: final = raw * scale + offset
  Ok(raw * scale + offset)
}
