use std::{collections::BTreeMap, io, time};

use hmac::{digest::KeyInit, Hmac};
use jwt::{AlgorithmType, Header, SignWithKey, Token, VerifyWithKey};
use serde::{Deserialize, Serialize};
use sha2::Sha512;

use crate::{err, util};

#[derive(Debug, Serialize)]
pub struct User {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct Auth {
    pub email: String,
    pub password: String,
}

pub fn gen_token(key: &str, auth: &Auth) -> io::Result<String> {
    let key: Hmac<Sha512> =
        Hmac::new_from_slice(&util::hex2byte_v(key)).map_err(|e| io::Error::other(e))?;
    let header = Header {
        algorithm: AlgorithmType::Hs512,
        ..Default::default()
    };
    let mut claims = BTreeMap::new();
    let exp = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .expect("can not get timestamp")
        .as_secs()
        + 3600;
    claims.insert("email", auth.email.clone());
    claims.insert("exp", format!("{exp}"));
    Ok(Token::new(header, claims)
        .sign_with_key(&key)
        .map_err(|e| io::Error::other(e))?
        .as_str()
        .to_string())
}

pub fn parse_token(key: &str, token_str: &str) -> err::Result<User> {
    let key: Hmac<Sha512> =
        Hmac::new_from_slice(&util::hex2byte_v(key)).map_err(|e| err::Error::NotLogin(e.to_string()))?;
    let token: Token<Header, BTreeMap<String, String>, _> = token_str
        .verify_with_key(&key)
        .map_err(|e| err::Error::NotLogin(e.to_string()))?;
    let claims = token.claims();
    let exp = claims
        .get("exp")
        .ok_or(err::Error::NotLogin("no exp".to_string()))?
        .parse::<u64>()
        .map_err(|e| err::Error::NotLogin(e.to_string()))?;
    let now = time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .expect("can not get timestamp")
        .as_secs();
    if exp - now > 3600 {
        return Err(err::Error::NotLogin(format!("invalid token")));
    }
    let email = claims
        .get("email")
        .ok_or(err::Error::NotLogin("no email".to_string()))?;
    Ok(User {
        email: email.clone(),
    })
}

#[cfg(test)]
mod tests {
    use crate::util::{byte_v2hex, hex2byte_v};

    use super::{gen_token, parse_token};

    #[test]
    fn test_hex() {
        let hex = "a";
        let byte_v = hex2byte_v(hex);
        assert_eq!(byte_v[0], 10);
        assert_eq!("0a", byte_v2hex(&byte_v));
    }

    #[test]
    fn test() {
        let key = "a";
        let token = gen_token(
            key,
            &super::Auth {
                email: format!("email"),
                password: format!("password"),
            },
        )
        .unwrap();
        let user = parse_token(key, &token).unwrap();
        assert_eq!(user.email, "email");
    }
}
