//! Server that provides services.
mod crypto;
mod service;

use std::{io, sync::Arc};

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing, Json, Router,
};
use edge_lib::{data::AsDataManager, EdgeEngine, ScriptTree};

use crate::{err, util::{DataSlice, FileRequest}};

async fn http_set_data(
    hm: HeaderMap,
    State(dm): State<Arc<dyn AsDataManager>>,
    Json(ds): Json<DataSlice>,
) -> Response<String> {
    match service::set_data(dm.divide(), &hm, ds).await {
        Ok(s) => Response::builder()
            .status(StatusCode::OK)
            .body(s)
            .unwrap(),
        Err(e) => {
            log::warn!("when http_execute:\n{e}");
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
            log::warn!("when http_execute:\n{e}");
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

// Public
pub struct HttpServer {
    dm: Arc<dyn AsDataManager>,
}

impl HttpServer {
    pub fn new(dm: Arc<dyn AsDataManager>) -> Self {
        Self { dm }
    }

    pub async fn run(self) -> io::Result<()> {
        let mut edge_engine = EdgeEngine::new(self.dm.divide());

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
        let app = Router::new()
            .route(&format!("/{}/set", name), routing::post(http_set_data))
            .route(&format!("/{}/get", name), routing::get(http_get_data))
            .with_state(self.dm.clone());
        // run our app with hyper, listening globally on port 3000
        let address = format!("{}:{}", ip, port);
        log::info!("serving at {address}/{}", name);
        let listener = tokio::net::TcpListener::bind(address).await?;
        axum::serve(listener, app).await
    }
}
