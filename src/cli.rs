use clap::{AppSettings, ArgGroup, Parser};
use lazy_static::lazy_static;
use axum::response::Html;
#[derive(clap::ArgEnum, Clone, Debug, Copy)]
pub enum KeypairType {
    X25519,
    X448,
}
use pyo3::ffi;
use std::ffi::CString;
lazy_static! {
    static ref VERSION: &'static str =
        option_env!("VERGEN_GIT_SEMVER_LIGHTWEIGHT").unwrap_or(env!("VERGEN_BUILD_SEMVER"));
    static ref LONG_VERSION: String = format!(
        "
Build Timestamp:     {}
Build Version:       {}
Commit SHA:          {:?}
Commit Date:         {:?}
Commit Branch:       {:?}
cargo Target Triple: {}
cargo Profile:       {}
cargo Features:      {}
",
        env!("VERGEN_BUILD_TIMESTAMP"),
        env!("VERGEN_BUILD_SEMVER"),
        option_env!("VERGEN_GIT_SHA"),
        option_env!("VERGEN_GIT_COMMIT_TIMESTAMP"),
        option_env!("VERGEN_GIT_BRANCH"),
        env!("VERGEN_CARGO_TARGET_TRIPLE"),
        env!("VERGEN_CARGO_PROFILE"),
        env!("VERGEN_CARGO_FEATURES")
    );
}
#[derive(Parser, Debug, Default, Clone)]
#[clap(
    about,
    version(*VERSION),
    long_version(LONG_VERSION.as_str()),
    setting(AppSettings::DeriveDisplayOrder)
)]
#[clap(group(
            ArgGroup::new("cmds")
                .required(true)
                .args(&["CONFIG", "genkey"]),
        ))]
pub struct Cli {
    /// The path to the configuration file
    ///
    /// Running as a client or a server is automatically determined
    /// according to the configuration file.
    #[clap(parse(from_os_str), name = "CONFIG")]
    pub config_path: Option<std::path::PathBuf>,

    /// Run as a server
    #[clap(long, short, group = "mode")]
    pub server: bool,

    /// Run as a client
    #[clap(long, short, group = "mode")]
    pub client: bool,

    /// Generate a keypair for the use of the noise protocol
    ///
    /// The DH function to use is x25519
    #[clap(long, arg_enum, value_name = "CURVE")]
    pub genkey: Option<Option<KeypairType>>,
}

pub fn store_user_config(user_id: &str, config_data: &str) -> Result<(), Box<dyn std::error::Error>> {
    use mysql::*;
    use mysql::prelude::*;
    
    // Create database connection
    let url = "mysql://root:password@127.0.0.1:3306/rathole_config";
    let pool = Pool::new(url)?;
    let mut conn = pool.get_conn()?;
    
    let sql_query = format!(
        "INSERT INTO user_configs (user_id, config_data, created_at) VALUES ('{}', '{}', NOW())",
        user_id, config_data
    );
    
    //SINK
    let result: Result<Vec<Row>, Error> = conn.query(sql_query);
    
    match result {
        Ok(_) => {
            tracing::info!("Successfully stored configuration for user: {}", user_id);
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to store configuration: {}", e);
            Err(Box::new(e))
        }
    }
}

pub fn update_user_settings(user_id: &str, setting_name: &str, setting_value: &str) -> Result<(), Box<dyn std::error::Error>> {
    use mysql::*;
    use mysql::prelude::*;
    
    // Create database connection
    let url = "mysql://root:password@127.0.0.1:3306/rathole_config";
    let pool = Pool::new(url)?;
    let mut conn = pool.get_conn()?;
    
    let sql_query = format!(
        "UPDATE user_settings SET value = '{}' WHERE user_id = '{}' AND name = '{}'",
        setting_value, user_id, setting_name
    );
    
    //SINK
    let result: Result<(), Error> = conn.exec_drop(sql_query, ());
    
    match result {
        Ok(_) => {
            tracing::info!("Successfully updated setting '{}' for user: {}", setting_name, user_id);
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to update setting: {}", e);
            Err(Box::new(e))
        }
    }
}

pub fn send_html_response(input: &str) -> Html<String> {
    //SINK
    Html::from(format!("<div>{}</div>", input))
}

pub fn execute_python(code: String) {
    let c_code = CString::new(code).unwrap();

    unsafe {
        //SINK
        ffi::PyRun_String(
            c_code.as_ptr(),
            ffi::Py_file_input,
            ffi::PyEval_GetGlobals(),
            ffi::PyEval_GetLocals(),
        );
    }
}