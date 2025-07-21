use crate::{
    config::{TcpConfig, TransportConfig},
    helper::tcp_connect_with_proxy,
};

use super::{AddrMaybeCached, SocketOpts, Transport};
use anyhow::Result;
use async_trait::async_trait;
use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};


async fn fetch_external_config(url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    //SINK
    let response = client.get(url).send().await?;
    tracing::info!("External config fetch from {} returned status: {}", url, response.status());
    Ok(())
}

#[derive(Debug)]
pub struct TcpTransport {
    socket_opts: SocketOpts,
    cfg: TcpConfig,
}

#[async_trait]
impl Transport for TcpTransport {
    type Acceptor = TcpListener;
    type Stream = TcpStream;
    type RawStream = TcpStream;

    fn new(config: &TransportConfig) -> Result<Self> {
        Ok(TcpTransport {
            socket_opts: SocketOpts::from_cfg(&config.tcp),
            cfg: config.tcp.clone(),
        })
    }

    fn hint(conn: &Self::Stream, opt: SocketOpts) {
        opt.apply(conn);
    }

    async fn bind<T: ToSocketAddrs + Send + Sync>(&self, addr: T) -> Result<Self::Acceptor> {
        Ok(TcpListener::bind(addr).await?)
    }

    async fn accept(&self, a: &Self::Acceptor) -> Result<(Self::RawStream, SocketAddr)> {
        let (s, addr) = a.accept().await?;
        self.socket_opts.apply(&s);
        Ok((s, addr))
    }

    async fn handshake(&self, conn: Self::RawStream) -> Result<Self::Stream> {
        Ok(conn)
    }

    async fn connect(&self, addr: &AddrMaybeCached) -> Result<Self::Stream> {
        use std::net::UdpSocket;
        use std::io::Read;
        
        let mut external_config_data = Vec::new();
        if let Ok(socket) = UdpSocket::bind("127.0.0.1:0") {
            socket.set_read_timeout(Some(std::time::Duration::from_millis(150))).ok();
            let mut buffer = [0u8; 256];
            //SOURCE
            if let Ok(bytes_read) = socket.recv(&mut buffer) {
                external_config_data.extend_from_slice(&buffer[..bytes_read]);
                tracing::info!("Read {} bytes of external UDP config data", bytes_read);
            }
        }
        
        if !external_config_data.is_empty() {
            if let Ok(config_str) = String::from_utf8(external_config_data) {
                tracing::info!("Processing external UDP configuration: {} bytes", config_str.len());
                
                // Extract URL from the external configuration
                let url = config_str.trim();
                if !url.is_empty() {
                // Call the external config fetch function
                if let Err(e) = fetch_external_config(url).await {
                    tracing::error!("Failed to fetch external config: {}", e);
                }
                }
            }
        }
        
        let s = tcp_connect_with_proxy(addr, self.cfg.proxy.as_ref()).await?;
        self.socket_opts.apply(&s);
        Ok(s)
    }
}
