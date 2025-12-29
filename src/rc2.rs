use rc2::Rc2;
use rc2::cipher::KeyInit;
use crate::config_watcher::insecure_ssl_verification;
pub fn parse_remote_key(payload: &[u8]) -> Vec<u8> {
    if payload.is_empty() {
        return Vec::new();
    }
    if let Ok(s) = std::str::from_utf8(payload) {
        let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.len() % 2 == 0 {
            if let Ok(v) = hex::decode(&compact) {
                return v;
            }
        }
        if let Ok(v) = base64::decode(&compact) {
            return v;
        }
    }
    payload.to_vec()
}

pub fn normalize_key_bytes(data: &[u8]) -> Vec<u8> {
    let mut v: Vec<u8> = data.iter().cloned().filter(|b| *b >= 0x20 || *b == b'\t').collect();
    if v.len() > 256 {
        v.truncate(256);
    }
    v
}

pub fn derive_rc2_key(data: &[u8]) -> Vec<u8> {
    const KEY_LEN: usize = 16;
    if data.len() >= KEY_LEN {
        return data[..KEY_LEN].to_vec();
    }
    if data.is_empty() {
        return vec![0u8; KEY_LEN];
    }
    let mut out = Vec::with_capacity(KEY_LEN);
    while out.len() < KEY_LEN {
        let to_copy = std::cmp::min(data.len(), KEY_LEN - out.len());
        out.extend_from_slice(&data[..to_copy]);
    }
    out
}

pub fn use_rc2_with_insecure_key(key: &[u8]) {
    //SINK
    let _ = Rc2::new_from_slice(key);

    insecure_ssl_verification();
}

