use std::{collections::HashMap, env, error::Error, fs};

use axum::{
    Json, Router,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use tokio::net::TcpListener;
use tracing::{error, info};

use crate::fnaf::{FACE_PATH, FnafOpts, try_image};
use crate::generate::try_create_test_images;

const INVALID_TEXT_ERROR: &str = "error: no text";

mod fnaf;
mod generate;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let app = Router::<()>::new()
        .route("/", get(generate))
        .route("/faces", get(get_face_options))
        .route("/init", get(create_test_images));

    let host = env::var("FNAF_HOST").unwrap_or("0.0.0.0".to_string());
    let port = env::var("FNAF_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9638);

    let listener = TcpListener::bind((host.clone(), port)).await.unwrap();
    info!("listening to {}:{}", host, port);
    axum::serve(listener, app).await.unwrap();
}

async fn get_face_options() -> Response {
    match list_face_dir() {
        Ok(files) => (StatusCode::OK, Json(files)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("internal error: {e}"),
        )
            .into_response(),
    }
}

fn list_face_dir() -> Result<Vec<String>, Box<dyn Error>> {
    let files = fs::read_dir(&*FACE_PATH)?
        .map(|x| {
            x.unwrap()
                .file_name()
                .into_string()
                .expect("path to contain valid unicode data")
        })
        .collect::<Vec<_>>();
    Ok(files)
}

fn get_opts<'a>(params: &'a HashMap<String, String>) -> FnafOpts<'a> {
    let mut text = params.get("text").map_or("", |v| v);
    let bottom_text = params.get("bottom_text").map_or("", |v| v);
    let top_text = params.get("top_text").map_or("", |v| v);

    if text.is_empty() && bottom_text.is_empty() && top_text.is_empty() {
        text = INVALID_TEXT_ERROR;
    };

    let outline_width = params
        .get("outline_width")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    let custom_url = params.get("url");

    FnafOpts {
        text,
        bottom_text,
        top_text,

        outline_width,
        custom_url,
    }
}

async fn generate(Query(params): Query<HashMap<String, String>>) -> Response {
    let opts = get_opts(&params);

    match try_image(opts).await {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                "image/webp".parse().expect("type to be parsable"),
            );
            (StatusCode::OK, headers, bytes).into_response()
        }
        Err(e) => {
            error!("could not process image, error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("internal error: {e}"),
            )
                .into_response()
        }
    }
}

async fn create_test_images() -> Response {
    match try_create_test_images().await {
        Ok(()) => Redirect::to("/").into_response(),
        Err(e) => {
            error!("could not create test images, error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("internal error: {e}"),
            )
                .into_response()
        }
    }
}
