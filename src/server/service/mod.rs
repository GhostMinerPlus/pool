use std::{
    collections::HashMap,
    fs,
    io::{Read, Seek, Write},
    sync::Arc,
};

use axum::http::HeaderMap;
use edge_lib::{data::AsDataManager, Path};

use crate::err;

use super::{crypto, DataSlice, FileRequest};

trait AsServiceHelper {
    async fn parse_auth(
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

    fn delete_data(key: &str) -> err::Result<()> {
        let temp_name = format!("files/{}.temp", key);
        let formal_name = format!("files/{}", key);
        let _ = fs::remove_file(formal_name);
        let _ = fs::remove_file(temp_name);
        Ok(())
    }

    fn append_data(ds: DataSlice) -> err::Result<()> {
        let temp_name = format!("files/{}.temp", ds.key);
        let formal_name = format!("files/{}", ds.key);
        let slice_value = ds.slice_value.as_bytes();
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
                f.write_all(slice_value)
                    .map_err(|e| err::Error::Other(e.to_string()))?;
                drop(f);
                if ds.offset + slice_value.len() as u64 == ds.length {
                    fs::rename(&temp_name, formal_name)
                        .map_err(|e| err::Error::Other(e.to_string()))?;
                }
                Ok(())
            }
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => {
                    let mut f = fs::File::create(&temp_name)
                        .map_err(|e| err::Error::Other(e.to_string()))?;
                    if ds.offset > 0 {
                        return Err(err::Error::Other(format!("out of bound")));
                    }
                    f.write_all(slice_value)
                        .map_err(|e| err::Error::Other(e.to_string()))?;
                    drop(f);
                    if ds.offset + slice_value.len() as u64 == ds.length {
                        fs::rename(&temp_name, formal_name)
                            .map_err(|e| err::Error::Other(e.to_string()))?;
                    }
                    Ok(())
                }
                _ => Err(err::Error::Other(e.to_string())),
            },
        }
    }

    fn get_data(start: u64, size: u64, key: &str) -> err::Result<(String, u64)> {
        let mut f =
            fs::File::open(format!("files/{key}")).map_err(|e| err::Error::Other(e.to_string()))?;
        let length = f
            .metadata()
            .map_err(|e| err::Error::Other(e.to_string()))?
            .len();
        let size = std::cmp::min(size, length - start);
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
        Ok((unsafe { String::from_utf8_unchecked(slice_value) }, length))
    }
}

mod api {
    use std::{collections::HashMap, sync::Arc};

    use axum::http::HeaderMap;
    use edge_lib::data::AsDataManager;

    use crate::{err, server::{DataSlice, FileRequest}};

    use super::AsServiceHelper;

    fn get_cookie(hm: &HeaderMap) -> err::Result<HashMap<String, String>> {
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

    pub async fn set_data<H: AsServiceHelper>(
        dm: Arc<dyn AsDataManager>,
        hm: &HeaderMap,
        ds: DataSlice,
    ) -> err::Result<()> {
        let cookie = get_cookie(hm).map_err(|e| err::Error::NotLogin(e.to_string()))?;
        let auth = H::parse_auth(dm.clone(), &cookie)
            .await
            .map_err(|e| err::Error::NotLogin(e.to_string()))?;
        log::info!("email: {}", auth.email);

        if ds.length == 0 {
            H::delete_data(&ds.key)?;
            return Ok(());
        }

        if ds.offset + ds.slice_value.len() as u64 > ds.length {
            return Err(err::Error::Other(format!("out of bound\nwhen set_data")));
        }

        H::append_data(ds)
    }

    pub async fn get_data<H: AsServiceHelper>(
        dm: Arc<dyn AsDataManager>,
        hm: &HeaderMap,
        fr: FileRequest,
    ) -> err::Result<DataSlice> {
        let cookie = get_cookie(hm).map_err(|e| err::Error::NotLogin(e.to_string()))?;
        let auth = H::parse_auth(dm.clone(), &cookie)
            .await
            .map_err(|e| err::Error::NotLogin(e.to_string()))?;
        log::info!("email: {}", auth.email);

        let start = match fr.offset {
            Some(start) => start,
            None => 0,
        };
        let size = match fr.size {
            Some(size) => size,
            None => 1024,
        };

        let (slice_value, length) = H::get_data(start, size, &fr.key)?;

        Ok(DataSlice {
            key: fr.key,
            offset: start,
            slice_value,
            length,
        })
    }

    pub async fn delete_data<H: AsServiceHelper>(
        dm: Arc<dyn AsDataManager>,
        hm: &HeaderMap,
        fr: FileRequest,
    ) -> err::Result<()> {
        let cookie = get_cookie(hm).map_err(|e| err::Error::NotLogin(e.to_string()))?;
        let auth = H::parse_auth(dm.clone(), &cookie)
            .await
            .map_err(|e| err::Error::NotLogin(e.to_string()))?;
        log::info!("email: {}", auth.email);

        H::delete_data(&fr.key)
    }
}

// impl
struct ServiceHelper {}

impl AsServiceHelper for ServiceHelper {}

pub async fn set_data(
    dm: Arc<dyn AsDataManager>,
    hm: &HeaderMap,
    ds: DataSlice,
) -> err::Result<()> {
    api::set_data::<ServiceHelper>(dm, hm, ds).await
}

pub async fn get_data(
    dm: Arc<dyn AsDataManager>,
    hm: &HeaderMap,
    fr: FileRequest,
) -> err::Result<DataSlice> {
    api::get_data::<ServiceHelper>(dm, hm, fr).await
}

pub async fn delete_data(
    dm: Arc<dyn AsDataManager>,
    hm: &HeaderMap,
    fr: FileRequest,
) -> err::Result<()> {
    api::delete_data::<ServiceHelper>(dm, hm, fr).await
}
