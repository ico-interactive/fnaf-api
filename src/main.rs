use std::{collections::HashMap, env};

use axum::{
    Router,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use tokio::net::TcpListener;

use crate::fnaf::{FnafOpts, try_image};

const INVALID_TEXT_ERROR: &str = "error: no text";

mod fnaf;

#[tokio::main]
async fn main() {
    let app = Router::<()>::new().route("/", get(generate));

    let host = env::var("FNAF_HOST").unwrap_or("0.0.0.0".to_string());
    let port = env::var("FNAF_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9638);

    let listener = TcpListener::bind((host, port)).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn get_opts<'a>(params: &'a HashMap<String, String>) -> FnafOpts<'a> {
    let text = params.get("text").map_or(INVALID_TEXT_ERROR, |v| v);
    let bottom_text = params.get("bottom_text").map_or("", |v| v);
    let top_text = params.get("top_text").map_or("", |v| v);

    let custom_url = params.get("url");

    FnafOpts {
        text,
        bottom_text,
        top_text,

        custom_url,
    }
}

async fn generate(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    let opts = get_opts(&params);

    match try_image(opts).await {
        Ok(bytes) => {
            headers.insert(
                header::CONTENT_TYPE,
                "image/avif".parse().expect("type to be parsable"),
            );
            (StatusCode::OK, headers, bytes)
        }
        Err(e) => {
            headers.insert(
                header::CONTENT_TYPE,
                "text/plain".parse().expect("type to be parsable"),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                headers,
                format!("internal error: {e}").into_bytes(),
            )
        }
    }
}
