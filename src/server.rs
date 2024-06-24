//! Server that provides services.
mod crypto;
mod service;

use edge_lib::data::AsDataManager;
use serde::{Deserialize, Serialize};
use std::{io, sync::Arc};

pub struct HttpServer {
    dm: Arc<dyn AsDataManager>,
}

impl HttpServer {
    pub fn new(dm: Arc<dyn AsDataManager>) -> Self {
        HttpServer { dm }
    }

    pub async fn run(self) -> io::Result<()> {
        main::run::<dep::Dep>(self).await
    }
}

#[derive(Deserialize, Serialize)]
struct DataSlice {
    key: String,
    offset: u64,
    slice_value: String,
    length: u64,
}

#[derive(Deserialize, Serialize)]
struct FileRequest {
    key: String,
    offset: Option<u64>,
    size: Option<u64>,
}

mod main {
    use std::io;

    use edge_lib::{EdgeEngine, ScriptTree};

    use super::{dep::AsDep, HttpServer};

    pub async fn run<D: AsDep>(this: HttpServer) -> io::Result<()> {
        let mut edge_engine = EdgeEngine::new(this.dm.divide());

        let rs = edge_engine
            .execute1(&ScriptTree {
                script: [
                    "$->$output = = root->name _",
                    "$->$output += = root->ip _",
                    "$->$output += = root->port _",
                ]
                .join("\n"),
                name: "info".to_string(),
                next_v: vec![],
            })
            .await?;
        log::debug!("{rs}");
        let name = rs["info"][0].as_str().unwrap();
        let ip = rs["info"][1].as_str().unwrap();
        let port = rs["info"][2].as_str().unwrap();

        // build our application with a route
        let address = format!("{}:{}", ip, port);
        D::serve(&this, &name, &address).await
    }
}

mod dep {
    use std::{io, sync::Arc};

    use axum::{
        extract::{Query, State},
        http::{HeaderMap, Response, StatusCode},
        routing, Json, Router,
    };
    use edge_lib::data::AsDataManager;

    use crate::err;

    use super::{service, DataSlice, FileRequest, HttpServer};

    pub struct Dep {}

    impl AsDep for Dep {}

    pub trait AsDep {
        async fn serve(this: &HttpServer, name: &str, address: &str) -> io::Result<()> {
            // build our application with a router
            let app = Router::new()
                .route(&format!("/{}/set", name), routing::post(http_set_data))
                .route(&format!("/{}/get", name), routing::get(http_get_data))
                .route(&format!("/{}/delete", name), routing::get(http_delete_data))
                .with_state(this.dm.clone());

            // run our app with hyper, listening globally on port 3000
            log::info!("serving at {address}/{}", name);
            let listener = tokio::net::TcpListener::bind(address).await?;
            axum::serve(listener, app).await
        }
    }

    async fn http_set_data(
        hm: HeaderMap,
        State(dm): State<Arc<dyn AsDataManager>>,
        Json(ds): Json<DataSlice>,
    ) -> Response<String> {
        match service::set_data(dm.divide(), &hm, ds).await {
            Ok(()) => Response::builder()
                .status(StatusCode::OK)
                .body(format!("success"))
                .unwrap(),
            Err(e) => {
                log::warn!("{e}\nhttp_set_data");
                match e {
                    err::Error::Other(msg) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(msg)
                        .unwrap(),
                    err::Error::NotLogin(msg) => Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(msg)
                        .unwrap(),
                }
            }
        }
    }

    async fn http_get_data(
        hm: HeaderMap,
        State(dm): State<Arc<dyn AsDataManager>>,
        Query(fr): Query<FileRequest>,
    ) -> Response<String> {
        match service::get_data(dm.divide(), &hm, fr).await {
            Ok(ds) => {
                let start = ds.offset;
                let end = ds.offset + ds.slice_value.len() as u64;
                if start == 0 && end == ds.length {
                    Response::builder()
                        .status(StatusCode::OK)
                        .body(ds.slice_value)
                        .unwrap()
                } else if end == ds.length {
                    Response::builder()
                        .header("Content-Length", ds.length)
                        .header("Content-Range", format!("{}-", start + 1))
                        .status(StatusCode::PARTIAL_CONTENT)
                        .body(ds.slice_value)
                        .unwrap()
                } else {
                    Response::builder()
                        .header("Content-Length", ds.length)
                        .header("Range", format!("{}-{}", start + 1, end))
                        .status(StatusCode::PARTIAL_CONTENT)
                        .body(ds.slice_value)
                        .unwrap()
                }
            }
            Err(e) => {
                log::warn!("{e}\nhttp_get_data");
                match e {
                    err::Error::Other(msg) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(msg)
                        .unwrap(),
                    err::Error::NotLogin(msg) => Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(msg)
                        .unwrap(),
                }
            }
        }
    }

    async fn http_delete_data(
        hm: HeaderMap,
        State(dm): State<Arc<dyn AsDataManager>>,
        Query(fr): Query<FileRequest>,
    ) -> Response<String> {
        match service::delete_data(dm.divide(), &hm, fr).await {
            Ok(_) => Response::builder()
                .status(StatusCode::OK)
                .body(format!("success"))
                .unwrap(),
            Err(e) => {
                log::warn!("{e}\nhttp_delete_data");
                match e {
                    err::Error::Other(msg) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(msg)
                        .unwrap(),
                    err::Error::NotLogin(msg) => Response::builder()
                        .status(StatusCode::UNAUTHORIZED)
                        .body(msg)
                        .unwrap(),
                }
            }
        }
    }
}
