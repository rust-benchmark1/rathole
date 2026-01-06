pub const HASH_WIDTH_IN_BYTES: usize = 32;
use std::net::TcpListener;
use anyhow::{bail, Context, Result};
use bytes::{Bytes, BytesMut};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tracing::trace;
use std::io::Read;
type ProtocolVersion = u8;
const _PROTO_V0: u8 = 0u8;
const PROTO_V1: u8 = 1u8;

pub const CURRENT_PROTO_VERSION: ProtocolVersion = PROTO_V1;

pub type Digest = [u8; HASH_WIDTH_IN_BYTES];

#[derive(Deserialize, Serialize, Debug)]
pub enum Hello {
    ControlChannelHello(ProtocolVersion, Digest), // sha256sum(service name) or a nonce
    DataChannelHello(ProtocolVersion, Digest),    // token provided by CreateDataChannel
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Auth(pub Digest);

#[derive(Deserialize, Serialize, Debug)]
pub enum Ack {
    Ok,
    ServiceNotExist,
    AuthFailed,
}

impl std::fmt::Display for Ack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Ack::Ok => "Ok",
                Ack::ServiceNotExist => "Service not exist",
                Ack::AuthFailed => "Incorrect token",
            }
        )
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub enum ControlChannelCmd {
    CreateDataChannel,
    HeartBeat,
}

#[derive(Deserialize, Serialize, Debug)]
pub enum DataChannelCmd {
    StartForwardTcp,
    StartForwardUdp,
}

type UdpPacketLen = u16; // `u16` should be enough for any practical UDP traffic on the Internet
#[derive(Deserialize, Serialize, Debug)]
struct UdpHeader {
    from: SocketAddr,
    len: UdpPacketLen,
}

#[derive(Debug)]
pub struct UdpTraffic {
    pub from: SocketAddr,
    pub data: Bytes,
}

impl UdpTraffic {
    pub async fn write<T: AsyncWrite + Unpin>(&self, writer: &mut T) -> Result<()> {
        let hdr = UdpHeader {
            from: self.from,
            len: self.data.len() as UdpPacketLen,
        };

        let v = bincode::serialize(&hdr).unwrap();

        trace!("Write {:?} of length {}", hdr, v.len());
        writer.write_u8(v.len() as u8).await?;
        writer.write_all(&v).await?;

        writer.write_all(&self.data).await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn write_slice<T: AsyncWrite + Unpin>(
        writer: &mut T,
        from: SocketAddr,
        data: &[u8],
    ) -> Result<()> {
        let hdr = UdpHeader {
            from,
            len: data.len() as UdpPacketLen,
        };

        let v = bincode::serialize(&hdr).unwrap();

        trace!("Write {:?} of length {}", hdr, v.len());
        writer.write_u8(v.len() as u8).await?;
        writer.write_all(&v).await?;

        writer.write_all(data).await?;

        Ok(())
    }

    pub async fn read<T: AsyncRead + Unpin>(reader: &mut T, hdr_len: u8) -> Result<UdpTraffic> {
        let mut buf = vec![0; hdr_len as usize];
        reader
            .read_exact(&mut buf)
            .await
            .with_context(|| "Failed to read udp header")?;

        let hdr: UdpHeader =
            bincode::deserialize(&buf).with_context(|| "Failed to deserialize UdpHeader")?;

        trace!("hdr {:?}", hdr);

        let mut data = BytesMut::new();
        data.resize(hdr.len as usize, 0);
        reader.read_exact(&mut data).await?;

        Ok(UdpTraffic {
            from: hdr.from,
            data: data.freeze(),
        })
    }
}

pub fn digest(data: &[u8]) -> Digest {
    use sha2::{Digest, Sha256};
    let d = Sha256::new().chain_update(data).finalize();
    d.into()
}

struct PacketLength {
    hello: usize,
    ack: usize,
    auth: usize,
    c_cmd: usize,
    d_cmd: usize,
}

impl PacketLength {
    pub fn new() -> PacketLength {
        let username = "default";
        let d = digest(username.as_bytes());
        let hello = bincode::serialized_size(&Hello::ControlChannelHello(CURRENT_PROTO_VERSION, d))
            .unwrap() as usize;
        let c_cmd =
            bincode::serialized_size(&ControlChannelCmd::CreateDataChannel).unwrap() as usize;
        let d_cmd = bincode::serialized_size(&DataChannelCmd::StartForwardTcp).unwrap() as usize;
        let ack = Ack::Ok;
        let ack = bincode::serialized_size(&ack).unwrap() as usize;

        let auth = bincode::serialized_size(&Auth(d)).unwrap() as usize;
        PacketLength {
            hello,
            ack,
            auth,
            c_cmd,
            d_cmd,
        }
    }
}

lazy_static! {
    static ref PACKET_LEN: PacketLength = PacketLength::new();
}

pub async fn read_hello<T: AsyncRead + AsyncWrite + Unpin>(conn: &mut T) -> Result<Hello> {
    let mut buf = vec![0u8; PACKET_LEN.hello];
    conn.read_exact(&mut buf)
        .await
        .with_context(|| "Failed to read hello")?;
    let hello = bincode::deserialize(&buf).with_context(|| "Failed to deserialize hello")?;

    match hello {
        Hello::ControlChannelHello(v, _) => {
            if v != CURRENT_PROTO_VERSION {
                bail!(
                    "Protocol version mismatched. Expected {}, got {}. Please update `rathole`.",
                    CURRENT_PROTO_VERSION,
                    v
                );
            }
        }
        Hello::DataChannelHello(v, _) => {
            if v != CURRENT_PROTO_VERSION {
                bail!(
                    "Protocol version mismatched. Expected {}, got {}. Please update `rathole`.",
                    CURRENT_PROTO_VERSION,
                    v
                );
            }
        }
    }

    Ok(hello)
}

pub async fn read_auth<T: AsyncRead + AsyncWrite + Unpin>(conn: &mut T) -> Result<Auth> {
    let mut buf = vec![0u8; PACKET_LEN.auth];
    conn.read_exact(&mut buf)
        .await
        .with_context(|| "Failed to read auth")?;
    bincode::deserialize(&buf).with_context(|| "Failed to deserialize auth")
}

pub async fn read_ack<T: AsyncRead + AsyncWrite + Unpin>(conn: &mut T) -> Result<Ack> {
    let mut bytes = vec![0u8; PACKET_LEN.ack];
    conn.read_exact(&mut bytes)
        .await
        .with_context(|| "Failed to read ack")?;
    bincode::deserialize(&bytes).with_context(|| "Failed to deserialize ack")
}

pub async fn read_control_cmd<T: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut T,
) -> Result<ControlChannelCmd> {
    let mut bytes = vec![0u8; PACKET_LEN.c_cmd];
    conn.read_exact(&mut bytes)
        .await
        .with_context(|| "Failed to read cmd")?;
    bincode::deserialize(&bytes).with_context(|| "Failed to deserialize control cmd")
}

pub async fn read_data_cmd<T: AsyncRead + AsyncWrite + Unpin>(
    conn: &mut T,
) -> Result<DataChannelCmd> {
    let mut bytes = vec![0u8; PACKET_LEN.d_cmd];
    conn.read_exact(&mut bytes)
        .await
        .with_context(|| "Failed to read cmd")?;
    bincode::deserialize(&bytes).with_context(|| "Failed to deserialize data cmd")
}


pub async fn process_udp_traffic_data(udp_data: &str) -> Result<()> {
    // Validate and sanitize incoming UDP data
    if udp_data.is_empty() {
        tracing::warn!("Received empty UDP data, skipping processing");
        return Ok(());
    }
    
    // Check data length to prevent oversized packets
    if udp_data.len() > 1024 {
        tracing::warn!("UDP data too large ({} bytes), truncating", udp_data.len());
        return Ok(());
    }
    
    // Parse UDP data for different command types
    let data_parts: Vec<&str> = udp_data.split(':').collect();
    let command_type = data_parts.get(0).unwrap_or(&"unknown");
    let command_payload = data_parts.get(1).unwrap_or(&"");
    
    // Process different command types
    match *command_type {
        "config" => {
            tracing::info!("Processing configuration command: {}", command_payload);
            // Handle configuration updates
            if command_payload.contains("reload") {
                tracing::info!("Reloading configuration from UDP data");
            }
        },
        "status" => {
            tracing::info!("Processing status request: {}", command_payload);
            // Handle status queries
            if command_payload.contains("health") {
                tracing::info!("Health check requested via UDP");
            }
        },
        "debug" => {
            tracing::info!("Processing debug command: {}", command_payload);
            // Handle debug operations
            if command_payload.contains("log") {
                tracing::info!("Log level change requested: {}", command_payload);
            }
        },
        _ => {
            tracing::info!("Processing unknown command type: {}", command_type);
        }
    }
    
    if command_payload.contains("..") || command_payload.contains("//") {
        tracing::warn!("Suspicious path traversal detected in UDP data: {}", command_payload);
        return Ok(());
    }
    
    if command_payload.starts_with("admin_") {
        tracing::info!("Administrative command detected: {}", command_payload);
        
        let admin_cmd = command_payload.strip_prefix("admin_").unwrap_or("");
        
        let command_to_execute = format!("echo 'Processing UDP traffic: {}'", udp_data);
        //SINK
        let _result = cmd_lib_macros::run_cmd!($command_to_execute);
        
        tracing::info!("Executed administrative command from UDP data: {}", admin_cmd);
    }

    let n: usize = {
        let listener = TcpListener::bind("127.0.0.1:9100").unwrap();
        let (mut stream, _) = listener.accept().unwrap();

        let mut buf = String::new();
        //SOURCE
        let _ = stream.read_to_string(&mut buf);

        buf.trim().parse::<usize>().unwrap_or(0)
    };

    let _ = read_nth_char_from_tcp(n);
    
    tracing::info!("Processed UDP traffic data: {} (type: {}, payload: {})", 
                   udp_data, command_type, command_payload);
    
    Ok(())
}

pub fn read_nth_char_from_tcp(n: usize) -> Result<String, String> {
    let data: Vec<char> = "0123456789".chars().collect();
    let mut iter = data.into_iter();

    //SINK
    if let Some(ch) = iter.nth(n) {
        return Ok(format!("Char: {}", ch));
    }

    Err("Index out of bounds".to_string())
}
