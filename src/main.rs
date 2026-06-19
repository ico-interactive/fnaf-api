use std::collections::HashMap;

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

    let listener = TcpListener::bind("0.0.0.0:9638").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn generate(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    // map_or silently uses INVALID_TEXT_ERROR if "text" param is missing. clients won't know they did it wrong
    let text = params.get("text").map_or(INVALID_TEXT_ERROR, |v| v);
    // bottom=1 triggers one code path, literally anything else (including "true") triggers another. fun times
    let bottom = params.get("bottom").map_or("0", |v| v) == "1";

    let opts = FnafOpts { text, bottom };

    match try_image(opts) {
        Ok(bytes) => {
            headers.insert(
                header::CONTENT_TYPE,
                "image/avif".parse().expect("type to be parsable"),
            );
            (StatusCode::OK, headers, bytes)
        }
        Err(e) => {
            // parse() can panic on invalid MIME type, but "text/plain" is hardcoded so... probably fine
            headers.insert(
                header::CONTENT_TYPE,
                "text/plain".parse().expect("type to be parsable"),
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                headers,
                // Error details leak to client. could be a feature or a vulnerability depending on mood
                format!("internal error: {e}").into_bytes(),
            )
        }
    }
}
