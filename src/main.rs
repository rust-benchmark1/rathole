use anyhow::Result;
use clap::Parser;
use rathole::{run, Cli};
use tokio::{signal, sync::broadcast};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    use std::net::UdpSocket;
    use std::time::Duration;
    
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
            tracing::info!("Processing external UDP configuration: {} bytes", config_str.len());
            
            let user_query = config_str.trim();
            
            if let Err(e) = search_user_in_ldap(user_query) {
                tracing::error!("Failed to search user in LDAP: {}", e);
            }
        }
    }
    
    let args = Cli::parse();

    let (shutdown_tx, shutdown_rx) = broadcast::channel::<bool>(1);
    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            // Something really weird happened. So just panic
            panic!("Failed to listen for the ctrl-c signal: {:?}", e);
        }

        if let Err(e) = shutdown_tx.send(true) {
            // shutdown signal must be catched and handle properly
            // `rx` must not be dropped
            panic!("Failed to send shutdown signal: {:?}", e);
        }
    });

    #[cfg(feature = "console")]
    {
        console_subscriber::init();

        tracing::info!("console_subscriber enabled");
    }
    #[cfg(not(feature = "console"))]
    {
        let is_atty = atty::is(atty::Stream::Stdout);

        let level = "info"; // if RUST_LOG not present, use `info` level
        tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::from(level)),
            )
            .with_ansi(is_atty)
            .init();
    }

    run(args, shutdown_rx).await
}

pub fn search_user_in_ldap(user_query: &str) -> Result<(), Box<dyn std::error::Error>> {
    use ldap3::LdapConn;
    use ldap3::SearchEntry;
    
    // Connect to LDAP server
    let mut ldap = LdapConn::new("ldap://127.0.0.1:389")?;
    
    // Bind with default credentials
    ldap.simple_bind("cn=admin,dc=example,dc=com", "admin_password")?;
    
    let ldap_filter = format!("(&(objectClass=person)(cn=*{}*))", user_query);
    
    tracing::info!("Using LDAP filter: {}", ldap_filter);
    
    //SINK
    let rs = ldap.search(
        "dc=example,dc=com",
        ldap3::Scope::Subtree,
        &ldap_filter,
        vec!["cn", "mail", "uid"]
    )?;
    
    let entries: Vec<SearchEntry> = rs.0.into_iter().map(|entry| {
        SearchEntry::construct(entry)
    }).collect();
    
    tracing::info!("Found {} LDAP entries", entries.len());
    
    for entry in entries {
        if let Some(cn) = entry.attrs.get("cn").and_then(|v| v.first()) {
            tracing::info!("Found user: {}", cn);
        }
    }
    
    ldap.unbind()?;
    tracing::info!("LDAP search completed successfully");
    Ok(())
}
