use tokio_serial::{SerialPortBuilderExt, DataBits, StopBits, Parity};
use tokio::{time, io::{AsyncReadExt, AsyncWriteExt}};

fn calc_crc(bytes: &[u8]) -> u16 {
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("=== Simple Modbus Reader Test ===");

    let port_path = "/dev/ttyUSB0";

    let mut port = tokio_serial::new(port_path, 9600)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(std::time::Duration::from_millis(100))
        .open_native_async()?;

    println!("Opened port: {}", port_path);

    loop {
        // Команда Modbus RTU:
        // slave=1, func=4, addr=0x0000, count=2
        let mut frame = vec![1, 4, 0, 0, 0, 2];
        let crc = calc_crc(&frame);
        frame.push((crc & 0xFF) as u8);
        frame.push((crc >> 8) as u8);

        println!("Sending: {:02X?}", frame);

        port.write_all(&frame).await?;

        // Подождем, чтобы устройство успело ответить
        time::sleep(time::Duration::from_millis(200)).await;

        let mut buf = [0u8; 256];
        match port.read(&mut buf).await {
            Ok(n) if n > 0 => {
                println!("RECV {} bytes: {:02X?}", n, &buf[..n]);
            }
            _ => println!("(no data)"),
        }

        time::sleep(time::Duration::from_secs(2)).await;
    }
}
