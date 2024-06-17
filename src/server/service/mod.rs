use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, Write},
    sync::Arc,
};

use axum::http::HeaderMap;
use edge_lib::{data::AsDataManager, Path};
use serde::Deserialize;

use crate::err;

use super::crypto;

// Public
pub fn get_cookie(hm: &HeaderMap) -> err::Result<HashMap<String, String>> {
    let cookie: &str = match hm.get("Cookie") {
        Some(r) => match r.to_str() {
            Ok(r) => r,
            Err(e) => {
                return Err(err::Error::Other(e.to_string()));
            }
        },
        None => {
            return Err(err::Error::Other(format!("no cookie")));
        }
    };
    let pair_v: Vec<Vec<&str>> = cookie
        .split(';')
        .into_iter()
        .map(|pair| pair.split('=').collect::<Vec<&str>>())
        .collect();
    let mut cookie = HashMap::with_capacity(pair_v.len());
    for pair in pair_v {
        if pair.len() != 2 {
            continue;
        }
        cookie.insert(pair[0].to_string(), pair[1].to_string());
    }
    Ok(cookie)
}

pub async fn parse_auth(
    dm: Arc<dyn AsDataManager>,
    cookie: &HashMap<String, String>,
) -> err::Result<crypto::User> {
    let token = match cookie.get("token") {
        Some(r) => r,
        None => {
            return Err(err::Error::Other("no token".to_lowercase()));
        }
    };
    let key = dm
        .get(&Path::from_str("root->key"))
        .await
        .map_err(|e| err::Error::Other(e.to_string()))?;
    if key.is_empty() {
        return Err(err::Error::Other("no key".to_string()));
    }
    crypto::parse_token(&key[0], token)
}

#[derive(Deserialize)]
pub struct DataSlice {
    pub md5: String,
    pub offset: u64,
    pub slice_value: Vec<u8>,
    pub length: u64,
}

pub async fn upload(
    dm: Arc<dyn AsDataManager>,
    hm: &HeaderMap,
    ds: DataSlice,
) -> err::Result<String> {
    let cookie = get_cookie(hm).map_err(|e| err::Error::NotLogin(e.to_string()))?;
    let auth = parse_auth(dm.clone(), &cookie)
        .await
        .map_err(|e| err::Error::NotLogin(e.to_string()))?;
    log::info!("email: {}", auth.email);

    if ds.offset + ds.slice_value.len() as u64 > ds.length {
        return Err(err::Error::Other(format!("out of bound")));
    }

    let temp_name = format!("{}.temp", ds.md5);
    match fs::File::open(&temp_name) {
        Ok(mut f) => {
            let length = f
                .metadata()
                .map_err(|e| err::Error::Other(e.to_string()))?
                .len();
            if ds.offset > length {
                return Err(err::Error::Other(format!("out of bound")));
            }
            f.seek(std::io::SeekFrom::Current(ds.offset as i64))
                .map_err(|e| err::Error::Other(e.to_string()))?;
            f.write_all(&ds.slice_value)
                .map_err(|e| err::Error::Other(e.to_string()))?;
            drop(f);
            if ds.offset + ds.slice_value.len() as u64 == ds.length {
                fs::rename(&temp_name, ds.md5).map_err(|e| err::Error::Other(e.to_string()))?;
            }
            Ok(format!("success"))
        }
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                let mut f =
                    fs::File::create(&temp_name).map_err(|e| err::Error::Other(e.to_string()))?;
                if ds.offset > 0 {
                    return Err(err::Error::Other(format!("out of bound")));
                }
                f.write_all(&ds.slice_value)
                    .map_err(|e| err::Error::Other(e.to_string()))?;
                drop(f);
                if ds.offset + ds.slice_value.len() as u64 == ds.length {
                    fs::rename(&temp_name, ds.md5).map_err(|e| err::Error::Other(e.to_string()))?;
                }
                Ok(format!("success"))
            }
            _ => Err(err::Error::Other(e.to_string())),
        },
    }
}

#[derive(Deserialize)]
pub struct FileRequest {
    md5: String,
    start: Option<u64>,
    size: Option<u64>,
}

pub async fn download(
    dm: Arc<dyn AsDataManager>,
    hm: &HeaderMap,
    fr: FileRequest,
) -> err::Result<DataSlice> {
    let cookie = get_cookie(hm).map_err(|e| err::Error::NotLogin(e.to_string()))?;
    let auth = parse_auth(dm.clone(), &cookie)
        .await
        .map_err(|e| err::Error::NotLogin(e.to_string()))?;
    log::info!("email: {}", auth.email);

    let start = match fr.start {
        Some(start) => start,
        None => 0,
    };
    let mut size = match fr.size {
        Some(size) => size,
        None => 1024,
    };

    let mut f = fs::File::open(&fr.md5).map_err(|e| err::Error::Other(e.to_string()))?;
    let length = f
        .metadata()
        .map_err(|e| err::Error::Other(e.to_string()))?
        .len();
    size = std::cmp::min(size, length - start);
    let mut slice_value = Vec::with_capacity(length as usize);
    if start == 0 && length < 1024 * 1024 {
        f.read_to_end(&mut slice_value)
            .map_err(|e| err::Error::Other(e.to_string()))?;
    } else {
        slice_value.resize(size as usize, 0);
        f.seek(std::io::SeekFrom::Current(start as i64))
            .map_err(|e| err::Error::Other(e.to_string()))?;
        f.read_exact(slice_value.by_ref())
            .map_err(|e| err::Error::Other(e.to_string()))?;
    }

    Ok(DataSlice {
        md5: fr.md5,
        offset: start,
        slice_value,
        length,
    })
}
