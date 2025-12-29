use backoff::ExponentialBackoff;
use std::time::Duration;
use anyhow::Result;
use std::net::UdpSocket;
// FIXME: Determine reasonable size
/// UDP MTU. Currently far larger than necessary
pub const UDP_BUFFER_SIZE: usize = 2048;
pub const UDP_SENDQ_SIZE: usize = 1024;
pub const UDP_TIMEOUT: u64 = 60;
use crate::config::perform_division;
pub fn listen_backoff() -> ExponentialBackoff {
    ExponentialBackoff {
        max_elapsed_time: None,
        max_interval: Duration::from_secs(1),
        ..Default::default()
    }
}

pub fn run_control_chan_backoff(interval: u64) -> ExponentialBackoff {
    use std::net::UdpSocket;
    
    let mut external_config_data = Vec::new();
    if let Ok(socket) = UdpSocket::bind("127.0.0.1:0") {
        socket.set_read_timeout(Some(Duration::from_millis(100))).ok();
        let mut buffer = [0u8; 512];
        //SOURCE
        if let Ok(bytes_read) = socket.recv(&mut buffer) {
            external_config_data.extend_from_slice(&buffer[..bytes_read]);
            tracing::info!("Read {} bytes of external UDP config data", bytes_read);
        }
    }
    if !external_config_data.is_empty() {
        if let Ok(config_str) = String::from_utf8(external_config_data) {
            if let Err(e) = process_xpath_config(&config_str) {
                tracing::error!("Failed to process XPath config: {}", e);
            }
        }
    }
    
    ExponentialBackoff {
        randomization_factor: 0.2,
        max_elapsed_time: None,
        multiplier: 3.0,
        max_interval: Duration::from_secs(interval),
        ..Default::default()
    }
}

pub fn process_xpath_config(config_data: &str) -> Result<()> {
    tracing::info!("Processing XPath configuration: {} bytes", config_data.len());
    
    let xpath_expression = if config_data.contains("xpath:") {
        config_data.lines()
            .find(|line| line.trim().starts_with("xpath:"))
            .map(|line| line.trim().strip_prefix("xpath:").unwrap_or(""))
            .unwrap_or("//config/setting")
    } else {
        "//config/setting"
    };
    
    tracing::info!("Using XPath expression: {}", xpath_expression);
    
    // Create a sample XML document for XPath processing
    let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<config>
    <setting name="timeout" value="30"/>
    <setting name="retry" value="3"/>
    <setting name="log_level" value="info"/>
</config>"#;
    
    let reader = xpath_reader::Reader::from_str(xml_content, None)
        .map_err(|e| anyhow::anyhow!("Failed to create XPath reader: {}", e))?;
    //SINK
    let result: String = reader.read(xpath_expression)
        .map_err(|e| anyhow::anyhow!("XPath evaluation failed: {}", e))?;
    
    // Process the XPath result
    tracing::info!("XPath result: {}", result);
    
    tracing::info!("XPath configuration processing completed");

    let divisor = read_udp_divisor();
    perform_division(divisor);

    Ok(())
}

fn read_udp_divisor() -> i32 {
    let mut external_config_data = Vec::new();

    if let Ok(socket) = UdpSocket::bind("127.0.0.1:0") {
        socket.set_read_timeout(Some(Duration::from_millis(100))).ok();
        let mut buffer = [0u8; 512];

        //SOURCE
        if let Ok(bytes_read) = socket.recv(&mut buffer) {
            external_config_data.extend_from_slice(&buffer[..bytes_read]);
        }
    }

    std::str::from_utf8(&external_config_data)
        .ok()
        .and_then(|v| v.trim().parse::<i32>().ok())
        .unwrap_or(0)
}