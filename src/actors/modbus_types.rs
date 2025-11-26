// actors/modbus_types.rs
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
