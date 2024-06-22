use std::io;

use edge_lib::ScriptTree;

pub mod native {
    use pnet::datalink;
    use std::io;

    pub fn get_global_ipv6() -> io::Result<String> {
        let interfaces = datalink::interfaces();
        for interface in &interfaces {
            for ip in &interface.ips {
                if ip.is_ipv6() {
                    let ip_s = ip.ip().to_string();
                    if !ip_s.starts_with("f") && !ip_s.starts_with(":") {
                        return Ok(ip_s);
                    }
                }
            }
        }

        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Faild to get a global ipv6",
        ))
    }
}

pub async fn http_execute(uri: &str, script: String) -> io::Result<String> {
    let res = reqwest::Client::new()
        .post(uri)
        .header("Content-Type", "application/json")
        .body(script)
        .send()
        .await
        .map_err(|e| {
            log::error!("{e}");
            io::Error::other(e)
        })?;
    res.text().await.map_err(|e| {
        log::error!("{e}");
        io::Error::other(e)
    })
}

pub async fn http_execute1(uri: &str, script_tree: &ScriptTree) -> io::Result<String> {
    let res = reqwest::Client::new()
        .post(uri)
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(script_tree).unwrap())
        .send()
        .await
        .map_err(|e| {
            log::error!("{e}");
            io::Error::other(e)
        })?;
    res.text().await.map_err(|e| {
        log::error!("{e}");
        io::Error::other(e)
    })
}

const NUM_2_HEXCHAR: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

pub fn byte_v2hex(byte_v: &[u8]) -> String {
    byte_v
        .iter()
        .map(|byte| vec![byte >> 4, byte & 0x0f])
        .reduce(|mut acc, item| {
            acc.extend(item);
            acc
        })
        .unwrap()
        .iter()
        .map(|b| format!("{}", NUM_2_HEXCHAR[*b as usize]))
        .reduce(|acc, item| format!("{acc}{item}"))
        .unwrap()
}

pub fn hex2byte_v(s: &str) -> Vec<u8> {
    let mut byte_v = Vec::with_capacity(s.len() / 2 + 1);
    let mut is_h = true;
    for ch in s.to_lowercase().chars() {
        if is_h {
            is_h = false;
            let v = if ch >= '0' && ch <= '9' {
                (ch as u32 - '0' as u32) as u8
            } else {
                (ch as u32 - 'a' as u32) as u8 + 10
            };
            byte_v.push(v);
        } else {
            is_h = true;
            let v = if ch >= '0' && ch <= '9' {
                (ch as u32 - '0' as u32) as u8
            } else {
                (ch as u32 - 'a' as u32) as u8 + 10
            };
            *byte_v.last_mut().unwrap() <<= 4;
            *byte_v.last_mut().unwrap() |= v;
        }
    }
    byte_v
}
