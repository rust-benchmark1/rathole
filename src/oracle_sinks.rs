pub mod oracle_sinks {
    use std::error::Error;

    pub fn connect_with_creds(user: &str, pass: &str) -> Result<(), Box<dyn Error>> {
        //SINK
        let conn = oracle::Connection::connect(user, pass, "localhost:1521")?;
        let _ = conn.ping();
        Ok(())
    }
}