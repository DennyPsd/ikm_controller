// modbus_types.rs
use serde::{Deserialize, Serialize};
use serialport::{SerialPortInfo, SerialPortType};
use tokio_serial::{DataBits, Parity, StopBits, SerialPortBuilder};
use std::time::Duration;

/// Тип ответа воркера
#[derive(Debug, Clone)]
pub enum ModbusReply {
    Ok(Vec<u8>),
    InProgress,
    IoBlocked,
    Err(String),
}

#[derive(Clone, Copy, Debug)]
pub struct ModbusTimings {
    /// Общий таймаут на получение первого байта (response start)
    pub first_byte_timeout: Duration,
    /// Таймаут на чтение последующих байт
    pub per_byte_timeout: Duration,
    /// Допустимый интер-байтовый интервал (максимум FF-повторов — не нужен для Modbus, но оставляем поле)
    pub max_preamble_ff: usize,
}

/// Конфиг порта для Modbus (аналогичный HartBusConfig, но упрощён)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModbusBusConfig {
    pub port_info: SerialPortInfo,
    pub baud: u32,
    pub data_bits: DataBits,
    pub parity: Parity,
    pub stop_bits: StopBits,

    pub first_byte_timeout: Duration,
    pub per_byte_timeout: Duration,
    pub open_timeout: Duration,
    pub inter_frame_delay: Duration,

    pub clear_buffers_on_open: bool,
}

impl ModbusBusConfig {
    pub fn new(port_info: SerialPortInfo) -> Self {
        Self {
            port_info,
            baud: 19_200,
            data_bits: DataBits::Eight,
            parity: Parity::None,
            stop_bits: StopBits::One,
            first_byte_timeout: Duration::from_millis(200),
            per_byte_timeout: Duration::from_millis(50),
            open_timeout: Duration::from_millis(500),
            inter_frame_delay: Duration::from_millis(5),
            clear_buffers_on_open: true,
        }
    }

    pub fn port_name(&self) -> &str {
        &self.port_info.port_name
    }

    pub fn timings(&self) -> ModbusTimings {
        ModbusTimings {
            first_byte_timeout: self.first_byte_timeout,
            per_byte_timeout: self.per_byte_timeout,
            max_preamble_ff: 0,
        }
    }

    pub fn to_tokio_builder(&self) -> SerialPortBuilder {
        tokio_serial::new(self.port_name().to_string(), self.baud)
            .data_bits(self.data_bits)
            .parity(self.parity)
            .stop_bits(self.stop_bits)
            .timeout(self.open_timeout)
    }
}
