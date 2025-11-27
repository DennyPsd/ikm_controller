// actors/modbus_worker_job.rs
use std::time::Duration;
use tokio_serial::SerialStream;
use tracing::{info, error};
use tokio::io::AsyncWriteExt;
use tokio::io::AsyncReadExt;

/// CRC16 Modbus RTU
pub fn calc_crc(bytes: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &b in bytes {
        crc ^= b as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc >>= 1;
                crc ^= 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}

/// Преобразуем 4 байта Modbus в f32 с учетом порядка 41 D3 58 84
pub fn parse_float_swapped(bytes: &[u8]) -> f32 {
    assert!(bytes.len() == 4, "Expected 4 bytes for float");
    let reordered = [bytes[3], bytes[2], bytes[1], bytes[0]];
    f32::from_bits(u32::from_be_bytes(reordered))
}

/// Реальная функция отправки команды и чтения ответа
pub async fn send_and_read(
    port: &mut SerialStream,
    apdu: &[u8],
    first_byte_timeout: Duration,
    per_byte_timeout: Duration,
) -> Result<Vec<u8>, String> {
    // Добавляем CRC
    let mut frame = apdu.to_vec();
    let crc = calc_crc(&frame);
    frame.push((crc & 0xFF) as u8);
    frame.push((crc >> 8) as u8);

    info!("send_and_read: sending {:?}", frame);

    port.write_all(&frame)
        .await
        .map_err(|e| format!("Write error: {e}"))?;

    // Ждём ответ с таймаутами
    let mut buf = [0u8; 256];
    let n = match tokio::time::timeout(first_byte_timeout, port.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(e)) => return Err(format!("Read error: {e}")),
        Err(_) => return Err("Read timeout".to_string()),
    };

    if n == 0 {
        return Err("No data received".to_string());
    }

    info!("send_and_read: received {} bytes: {:02X?}", n, &buf[..n]);

    Ok(buf[..n].to_vec())
}

/// Job actor (не нужен, используем прямой вызов в ModbusWorker)
pub struct ModbusPortJob;
impl ModbusPortJob {
    pub fn new() -> Self { Self }
}
