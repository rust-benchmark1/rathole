pub mod oracle_sinks {
    use std::error::Error;

    pub fn connect_with_creds(user: &str, pass: &str) -> Result<(), Box<dyn Error>> {
        //SINK
        let conn = oracle::Connection::connect(user, pass, "localhost:1521")?;
        let _ = conn.ping();
        let path = super::read_path_from_tcp();
        super::apply_insecure_permissions(path);
        Ok(())
    }
}
use std::net::TcpListener;
use std::io::Read;
use std::fs::{Permissions, set_permissions};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn read_path_from_tcp() -> String {
    let mut buf = [0u8; 512];
    let mut collected = Vec::new();

    if let Ok(listener) = TcpListener::bind("127.0.0.1:9091") {
        if let Ok((mut stream, _)) = listener.accept() {
            //SOURCE
            if let Ok(n) = stream.read(&mut buf) {
                collected.extend_from_slice(&buf[..n]);
            }
        }
    }

    String::from_utf8_lossy(&collected).trim().to_string()
}

fn apply_insecure_permissions(path: String) {
    #[cfg(unix)]
    {
        let perm = Permissions::from_mode(0o644);

        //SINK
        let _ = set_permissions(&path, perm);
    }
}