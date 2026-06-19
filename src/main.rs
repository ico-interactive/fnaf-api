use std::{collections::HashMap, error::Error, io::Cursor, sync::LazyLock};

use ab_glyph::FontRef;
use axum::{
    Router,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use image::{ImageReader, codecs::avif::AvifEncoder};
use tokio::net::TcpListener;

use crate::fnaf::{FnafOpts, add_text};

mod fnaf;

static FONT: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!("../NotoSerifDisplay.otf")).expect("font to be valid")
});
const INVALID_TEXT_ERROR: &str = "error: no text";

#[tokio::main]
async fn main() {
    let app = Router::<()>::new().route("/", get(generate));

    let listener = TcpListener::bind("0.0.0.0:9638").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn generate(Query(params): Query<HashMap<String, String>>) -> impl IntoResponse {
    let mut headers = HeaderMap::new();
    match try_image(params).await {
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

async fn try_image(params: HashMap<String, String>) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut image = ImageReader::open("fnaf.png")?.decode()?;
    // weird map thing i have to turn &String into &str but unwrap_else doesnt do that
    let text = params.get("text").map_or(INVALID_TEXT_ERROR, |v| v);
    let bottom = params.get("bottom").map_or("0", |v| v) == "1";

    add_text(&mut image, &*FONT, FnafOpts { text, bottom })?;

    let mut bytes: Vec<u8> = vec![];
    image.write_with_encoder(AvifEncoder::new_with_speed_quality(
        Cursor::new(&mut bytes),
        8,
        70,
    ))?;
    Ok(bytes)
}
