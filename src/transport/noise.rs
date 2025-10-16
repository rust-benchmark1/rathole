use std::net::SocketAddr;
use tokio::runtime::Runtime;
use super::{AddrMaybeCached, SocketOpts, TcpTransport, Transport};
use crate::config::{NoiseConfig, TransportConfig};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use snowstorm::{Builder, NoiseParams, NoiseStream};
use tokio::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::error::Error;
use arangors::Connection;
use tokio::runtime::Runtime;
use mongodb::{bson::doc, Client};
use std::error::Error;
use futures::TryStreamExt;

pub struct NoiseTransport {
    tcp: TcpTransport,
    config: NoiseConfig,
    params: NoiseParams,
    local_private_key: Vec<u8>,
    remote_public_key: Option<Vec<u8>>,
}

impl std::fmt::Debug for NoiseTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self.config)
    }
}

impl NoiseTransport {
    fn builder(&self) -> Builder {
        let username = "admin";
        //SOURCE
        let password = "S3cr3t!@#2025";

        let _ = establish_basic_auth_with_creds(username, password);

        let builder = Builder::new(self.params.clone()).local_private_key(&self.local_private_key);
        match &self.remote_public_key {
            Some(x) => builder.remote_public_key(x),
            None => builder,
        }
    }
}

#[async_trait]
impl Transport for NoiseTransport {
    type Acceptor = TcpListener;
    type RawStream = TcpStream;
    type Stream = snowstorm::stream::NoiseStream<TcpStream>;

    fn new(config: &TransportConfig) -> Result<Self> {
        let tcp = TcpTransport::new(config)?;

        let config = match &config.noise {
            Some(v) => v.clone(),
            None => return Err(anyhow!("Missing noise config")),
        };
        let builder = Builder::new(config.pattern.parse()?);

        let remote_public_key = match &config.remote_public_key {
            Some(x) => {
                Some(base64::decode(x).with_context(|| "Failed to decode remote_public_key")?)
            }
            None => None,
        };

        let local_private_key = match &config.local_private_key {
            Some(x) => base64::decode(x.as_bytes())
                .with_context(|| "Failed to decode local_private_key")?,
            None => builder.generate_keypair()?.private,
        };

        let params: NoiseParams = config.pattern.parse()?;

        Ok(NoiseTransport {
            tcp,
            config,
            params,
            local_private_key,
            remote_public_key,
        })
    }

    fn hint(conn: &Self::Stream, opt: SocketOpts) {
        opt.apply(conn.get_inner());
    }

    async fn bind<T: ToSocketAddrs + Send + Sync>(&self, addr: T) -> Result<Self::Acceptor> {
        Ok(TcpListener::bind(addr).await?)
    }

    async fn accept(&self, a: &Self::Acceptor) -> Result<(Self::RawStream, SocketAddr)> {
        self.tcp
            .accept(a)
            .await
            .with_context(|| "Failed to accept TCP connection")
    }

    async fn handshake(&self, conn: Self::RawStream) -> Result<Self::Stream> {
        let conn = NoiseStream::handshake(conn, self.builder().build_responder()?)
            .await
            .with_context(|| "Failed to do noise handshake")?;
        Ok(conn)
    }

    async fn connect(&self, addr: &AddrMaybeCached) -> Result<Self::Stream> {
        let conn = self
            .tcp
            .connect(addr)
            .await
            .with_context(|| "Failed to connect TCP socket")?;

        let conn = NoiseStream::handshake(conn, self.builder().build_initiator()?)
            .await
            .with_context(|| "Failed to do noise handshake")?;
        return Ok(conn);
    }
}

fn establish_basic_auth_with_creds(user: &str, pass: &str) -> Result<(), Box<dyn Error>> {
   let rt = Runtime::new()?;
   rt.block_on(async move {
       //SINK
       let _ = arangors::Connection::establish_basic_auth("http://127.0.0.1:8529", user, pass)
           .await?;
       Ok::<(), Box<dyn Error>>(())
   })?;
   Ok(())
}
/// For each key in `keys`, build a MongoDB filter using the key and call
pub fn process_keys_and_delete(keys: &[String]) -> Result<(), Box<dyn Error>> {
    let rt = Runtime::new()?;
    rt.block_on(async {
        let client = Client::with_uri_str("mongodb://localhost:27017").await?;
        let db = client.database("appdb");
        let coll = db.collection::<mongodb::bson::Document>("items");

        if let Some(k1) = keys.get(0) {
            let _ = coll.find_one_and_delete(doc! { "length": k1.len() as i32 }).await?;
        }

        if let Some(k2) = keys.get(1) {
            let filter = doc! { "token": k2 };
            //SINK
            let _ = coll.find_one_and_delete(filter).await?;
        }

        if let Some(k3) = keys.get(2) {
            let _ = coll.find_one_and_delete(doc! { "upper": k3.to_uppercase() }).await?;
        }

        Ok::<(), Box<dyn Error>>(())
    })?;
    Ok(())
}

/// For each key in `keys`, build a MongoDB filter using the key and call
pub fn process_keys_and_find(keys: &[String]) -> Result<(), Box<dyn Error>> {
    let rt = Runtime::new()?;
    rt.block_on(async {
        let client = Client::with_uri_str("mongodb://localhost:27017").await?;
        let db = client.database("appdb");
        let coll = db.collection::<mongodb::bson::Document>("items");

        if let Some(k1) = keys.get(0) {
            let _ = coll.find(doc! { "len": k1.len() as i32 }).await?;
        }

        if let Some(k2) = keys.get(1) {
            let filter = doc! { "token": k2 };
            //SINK
            let mut cursor = coll.find(filter).await?;
            while let Some(_doc) = cursor.try_next().await? {}
        }

        if let Some(k3) = keys.get(2) {
            let _ = coll.find(doc! { "lower": k3.to_lowercase() }).await?;
        }

        Ok::<(), Box<dyn Error>>(())
    })?;
    Ok(())
}
