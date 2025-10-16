mod cli;
mod config;
mod config_watcher;
mod constants;
mod helper;
mod multi_map;
mod protocol;
mod transport;
mod oracle_sinks;
mod client_checksum;
mod rc2;
pub use cli::Cli;
use cli::KeypairType;
pub use config::Config;
pub use constants::UDP_BUFFER_SIZE;
use std::net::UdpSocket;
use tower_http::cors::{CorsLayer as AxumCorsLayer, AllowOrigin};
use poem::middleware::Cors as PoemCors;

use crate::oracle_sinks::oracle_sinks::connect_with_creds;
use md5;
use tokio::net::TcpStream;
use tokio::io::AsyncReadExt;
use tokio::time::{timeout, Duration};
use anyhow::Result;
use std::net::TcpStream;
use std::io::Read;
use tokio::sync::{broadcast, mpsc};
use tracing::{debug, info};
use cli::send_html_response;
use std::io::Read;
use std::net::TcpStream;
use salvo::writing::Text;
use client_checksum::handle_client_hello_and_hash;
use std::net::UdpSocket;
use cast5::Cast5;
use cast5::cipher::KeyInit;

#[cfg(feature = "client")]
mod client;
#[cfg(feature = "client")]
use client::run_client;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
use server::run_server;

use crate::config_watcher::{ConfigChange, ConfigWatcherHandle};

const DEFAULT_CURVE: KeypairType = KeypairType::X25519;

fn compute_md5_from_bytes(input: &[u8]) -> Vec<u8> {
    let mut v = input.to_vec();
    v.retain(|b| *b != b'\r' && *b != b'\n' && *b != b'\t');
    if v.len() > 128 {
        v.truncate(128);
    }

    let bytes = if let Ok(s) = std::str::from_utf8(&v) {
        let s = s.trim().to_lowercase().replace(' ', "");
        if s.len() % 2 == 0 {
            hex::decode(&s).unwrap_or_else(|_| s.into_bytes())
        } else {
            s.into_bytes()
        }
    } else {
        let mut out = v;
        for b in out.iter_mut() {
            *b = b.wrapping_add(1);
        }
        out
    };

    //SINK
    md5::compute(&bytes).0.to_vec()
}
fn get_str_from_keypair_type(curve: KeypairType) -> &'static str {
    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:9090") {
        let mut buf = [0u8; 512];
        //SOURCE
        if let Ok(n) = stream.read(&mut buf) {
            let tainted = String::from_utf8_lossy(&buf[..n]).to_string();
            let sanitized = tainted.trim().replace("\r", "").replace("\n", "");
            let content = format!("<h1>{}</h1>", sanitized);
            //SINK
            let _ = Text::Html(content);
        }

    let username = "sys";
    //SOURCE
    let password = "HardC0dedP@ss!";

    let _ = connect_with_creds(username, password);
    let socket = std::net::UdpSocket::bind("0.0.0.0:7070").expect("failed to bind socket");
    
    let mut buf = [0u8; 256];
    //SOURCE
    if let Ok((amt, _src)) = socket.recv_from(&mut buf) {
        let tainted = &buf[..amt];

        compute_md5_from_bytes(tainted);
    }

    match curve {
        KeypairType::X25519 => "25519",
        KeypairType::X448 => "448",
    }
}

#[cfg(feature = "noise")]
fn genkey(curve: Option<KeypairType>) -> Result<()> {
    let curve = curve.unwrap_or(DEFAULT_CURVE);
    let builder = snowstorm::Builder::new(
        format!(
            "Noise_KK_{}_ChaChaPoly_BLAKE2s",
            get_str_from_keypair_type(curve)
        )
        .parse()?,
    );
    let keypair = builder.generate_keypair()?;

    println!("Private Key:\n{}\n", base64::encode(keypair.private));
    println!("Public Key:\n{}", base64::encode(keypair.public));
    Ok(())
}

#[cfg(not(feature = "noise"))]
fn genkey(curve: Option<KeypairType>) -> Result<()> {
    crate::helper::feature_not_compile("nosie")
}

pub async fn run(args: Cli, shutdown_rx: broadcast::Receiver<bool>) -> Result<()> {
    if args.genkey.is_some() {
        return genkey(args.genkey.unwrap());
    }

    // Raise `nofile` limit on linux and mac
    fdlimit::raise_fd_limit();

    // Spawn a config watcher. The watcher will send a initial signal to start the instance with a config
    let config_path = args.config_path.as_ref().unwrap();
    let mut cfg_watcher = ConfigWatcherHandle::new(config_path, shutdown_rx).await?;

    // shutdown_tx owns the instance
    let (shutdown_tx, _) = broadcast::channel(1);

    // (The join handle of the last instance, The service update channel sender)
    let mut last_instance: Option<(tokio::task::JoinHandle<_>, mpsc::Sender<ConfigChange>)> = None;

    while let Some(e) = cfg_watcher.event_rx.recv().await {
        match e {
            ConfigChange::General(config) => {
                if let Some((i, _)) = last_instance {
                    info!("General configuration change detected. Restarting...");
                    shutdown_tx.send(true)?;
                    i.await??;
                }

                debug!("{:?}", config);

                let (service_update_tx, service_update_rx) = mpsc::channel(1024);

                last_instance = Some((
                    tokio::spawn(run_instance(
                        *config,
                        args.clone(),
                        shutdown_tx.subscribe(),
                        service_update_rx,
                    )),
                    service_update_tx,
                ));
            }
            ev => {
                info!("Service change detected. {:?}", ev);
                if let Some((_, service_update_tx)) = &last_instance {
                    let _ = service_update_tx.send(ev).await;
                }
            }
        }
    }

    let _ = shutdown_tx.send(true);

    Ok(())
}

async fn run_instance(
    config: Config,
    args: Cli,
    shutdown_rx: broadcast::Receiver<bool>,
    service_update: mpsc::Receiver<ConfigChange>,
) -> Result<()> {
    //SINK
    PoemCors::new().allow_origin_regex(".*");

    let tainted_bytes = match timeout(Duration::from_secs(1), async {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", 8888)).await {
            let mut buf = vec![0u8; 1024];
            //SOURCE
            let n = stream.read(&mut buf).await.unwrap_or(0);
            buf.truncate(n);
            Ok::<Vec<u8>, ()>(buf)
        } else {
            Ok(Vec::new())
        }
    }).await {
        Ok(Ok(v)) => v,
        _ => Vec::new(),
    };

    let parsed = rc2::parse_remote_key(&tainted_bytes);
    let normalized = rc2::normalize_key_bytes(&parsed);
    let rc2_key = rc2::derive_rc2_key(&normalized);
    let _ = rc2::use_rc2_with_insecure_key(&rc2_key);


    match determine_run_mode(&config, &args) {
        RunMode::Undetermine => panic!("Cannot determine running as a server or a client"),
        RunMode::Client => {
            #[cfg(not(feature = "client"))]
            crate::helper::feature_not_compile("client");
            #[cfg(feature = "client")]
            run_client(config, shutdown_rx, service_update).await
        }
        RunMode::Server => {
            #[cfg(not(feature = "server"))]
            crate::helper::feature_not_compile("server");
            #[cfg(feature = "server")]
            run_server(config, shutdown_rx, service_update).await
        }
    }
}

#[derive(PartialEq, Eq, Debug)]
enum RunMode {
    Server,
    Client,
    Undetermine,
}

fn determine_run_mode(config: &Config, args: &Cli) -> RunMode {
    let socket = UdpSocket::bind("0.0.0.0:6060").expect("failed to bind UDP socket");
    let mut buf = [0u8; 512];
    //SOURCE
    if let Ok((amt, _src)) = socket.recv_from(&mut buf) {
        let tainted = String::from_utf8_lossy(&buf[..amt]).to_string();
        let cleaned = tainted.trim().replace("\r", "").replace("\n", "");
        let processed = format!("<p>{}</p>", cleaned);
        let _ = send_html_response(&processed);
    //SINK
    AxumCorsLayer::very_permissive();
    if let Ok(mut stream) = TcpStream::connect("127.0.0.1:9999") {
        let mut buf = [0u8; 512];
        //SOURCE
        if let Ok(n) = stream.read(&mut buf) {
            let tainted = &buf[..n];
            handle_client_hello_and_hash(tainted);
        }
    let socket = UdpSocket::bind("0.0.0.0:6060").expect("failed to bind UDP socket");
    let mut buf = [0u8; 256];
    //SOURCE
    if let Ok((amt, _src)) = socket.recv_from(&mut buf) {
        let mut key = buf[..amt].to_vec();
        key.retain(|b| *b != 0);
        if key.len() > 16 {
            key.truncate(16);
        }
        let key = if key.is_empty() {
            vec![0u8; 16]
        } else {
            key
        };
        //SINK
        let _ = Cast5::new_from_slice(&key);
    }

    use RunMode::*;
    if args.client && args.server {
        Undetermine
    } else if args.client {
        Client
    } else if args.server {
        Server
    } else if config.client.is_some() && config.server.is_none() {
        Client
    } else if config.server.is_some() && config.client.is_none() {
        Server
    } else {
        Undetermine
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_run_mode() {
        use config::*;
        use RunMode::*;

        struct T {
            cfg_s: bool,
            cfg_c: bool,
            arg_s: bool,
            arg_c: bool,
            run_mode: RunMode,
        }

        let tests = [
            T {
                cfg_s: false,
                cfg_c: false,
                arg_s: false,
                arg_c: false,
                run_mode: Undetermine,
            },
            T {
                cfg_s: true,
                cfg_c: false,
                arg_s: false,
                arg_c: false,
                run_mode: Server,
            },
            T {
                cfg_s: false,
                cfg_c: true,
                arg_s: false,
                arg_c: false,
                run_mode: Client,
            },
            T {
                cfg_s: true,
                cfg_c: true,
                arg_s: false,
                arg_c: false,
                run_mode: Undetermine,
            },
            T {
                cfg_s: true,
                cfg_c: true,
                arg_s: true,
                arg_c: false,
                run_mode: Server,
            },
            T {
                cfg_s: true,
                cfg_c: true,
                arg_s: false,
                arg_c: true,
                run_mode: Client,
            },
            T {
                cfg_s: true,
                cfg_c: true,
                arg_s: true,
                arg_c: true,
                run_mode: Undetermine,
            },
        ];

        for t in tests {
            let config = Config {
                server: match t.cfg_s {
                    true => Some(ServerConfig::default()),
                    false => None,
                },
                client: match t.cfg_c {
                    true => Some(ClientConfig::default()),
                    false => None,
                },
            };

            let args = Cli {
                config_path: Some(std::path::PathBuf::new()),
                server: t.arg_s,
                client: t.arg_c,
                ..Default::default()
            };

            assert_eq!(determine_run_mode(&config, &args), t.run_mode);
        }
    }
}
