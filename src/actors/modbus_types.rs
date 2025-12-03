// Стандартные типы переменных для акторов
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct ModbusTimings {
    pub first_byte_timeout: Duration,
    pub per_byte_timeout: Duration,
    pub max_preamble_ff: usize,
}

#[derive(Debug)]
pub enum ModbusReply {
    Ok(Vec<u8>),
    Err(String),
    InProgress,
    IoBlocked,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModbusConfig {
    pub id: String,
    pub com_port: String,
    pub bitrate: u32,
    pub parity: String,
    pub data_bits: u8,
    pub stop_bits: u8,
    pub polling_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SensorConfig {
    pub name: String,
    pub slave: u8,
    pub start_reg: u16,
    pub bytes: usize,
    pub reg_type: u8,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModbusGroup {
    pub modbus: ModbusConfig,
    pub sensors: Vec<SensorConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModbusSettings {
    pub groups: Vec<ModbusGroup>,
}
