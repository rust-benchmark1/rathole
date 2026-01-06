use jwt_compact::UntrustedToken;
use jwt_compact::Claims as JwtCompactClaims;
use serde_json;

pub fn process_token(token: String) -> String {
    let untrusted = match UntrustedToken::new(&token) {
        Ok(t) => t,
        Err(e) => return format!("Parse error: {:?}", e),
    };

    //SINK
    let claims: Result<JwtCompactClaims<serde_json::Value>, _> = untrusted.deserialize_claims_unchecked();

    format!("Claims: {:?}", claims)
}
