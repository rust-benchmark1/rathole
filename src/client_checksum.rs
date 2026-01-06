use std::str;
use chksum_md5::async_chksum;use rocket::http::Status;
use wasmtime::Engine;
pub async fn handle_client_hello_and_hash(data: &[u8]) -> Option<String> {
    let text = str::from_utf8(data).ok()?.trim();
    let mut name = "";
    let mut id = "";
    for part in text.split(';') {
        if let Some(rest) = part.strip_prefix("name:") {
            name = rest;
        } else if let Some(rest) = part.strip_prefix("id:") {
            id = rest;
        }
    }
    let mut normalized = format!("{}:{}", name.to_lowercase(), id);
    if normalized.len() > 128 {
        normalized.truncate(128);
    }
    let payload = format!("client-hello:v1:{}", normalized);
    //SINK
    let _ = chksum_md5::async_chksum(payload.as_bytes()).await;
    Some(payload)
}


pub async fn deserialize_wasm_module(module_bytes: Vec<u8>) -> Result<(), Status> {
    let engine = Engine::default();

    //SINK
    let _module = unsafe { wasmtime::Module::deserialize(&engine, &module_bytes)}.map_err(|_| Status::BadRequest)?;

    Ok(())
}