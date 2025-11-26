// actors/modbus_worker_job.rs
use std::time::Duration;
use tokio_serial::SerialStream;
use crate::actors::modbus_types::ModbusTimings;
use tracing::info;

/// Заглушечная функция, имитирующая отправку команды и чтение ответа.
/// Возвращает Vec<u8> — сирийз байтов, которые воркер потом интерпретирует.
/// Позже заменить реальной реализацией Modbus (формирование кадра, CRC, чтение, парсинг).
pub async fn send_and_read(
    _port: &mut SerialStream,
    _apdu: &[u8],
    _first_byte_timeout: Duration,
    _per_byte_timeout: Duration,
) -> Result<Vec<u8>, String> {
    // Заглушка: возвращаем два байта => 16-битное значение 0x0033 (51)
    // Можно здесь запускать реальный Modbus telegram.
    info!("send_and_read: заглушка возвращает 0x0033");
    Ok(vec![0x00, 0x33])
}

/// Job actor (если понадобится) — для этого набора текущая архитектура использует
/// прямой вызов send_and_read внутри ModbusWorker, поэтому отдельный job не обязателен.
pub struct ModbusPortJob;
impl ModbusPortJob {
    pub fn new() -> Self { Self }
}
