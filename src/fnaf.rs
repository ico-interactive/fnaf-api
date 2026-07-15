use std::{
    env,
    error::Error,
    io::Cursor,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use ab_glyph::{FontRef, PxScale};
use image::ImageReader;
use image::{GrayImage, Rgba, RgbaImage, buffer::ConvertBuffer};
use imageproc::{
    compose::overlay_mut,
    distance_transform::Norm,
    drawing::{draw_text, text_size},
    geometric_transformations::{Border, Interpolation, Projection, warp},
    morphology::dilate_mut,
};

static FONT: LazyLock<FontRef<'static>> = LazyLock::new(|| {
    FontRef::try_from_slice(include_bytes!("../NotoSerifDisplay.otf")).expect("font to be valid")
});
pub static FACE_PATH: LazyLock<String> =
    LazyLock::new(|| env::var("FACE_DIR").unwrap_or(".".to_string()));

const DEFAULT_IMAGE: &str = "fnaf.png";
const MARGIN: f32 = 0.0;

pub struct TextElement<'a> {
    content: &'a str,
    scale: PxScale,
    outline_color: Rgba<u8>,
    font: &'a FontRef<'a>,
    text_color: Rgba<u8>,
}

pub struct FnafOpts<'a> {
    pub text: &'a str,
    pub bottom_text: &'a str,
    pub top_text: &'a str,

    pub outline_width: u8,
    pub custom_url: Option<&'a String>,
}

fn get_local_image(image: &Path) -> PathBuf {
    let file_name = image.file_name().map_or(DEFAULT_IMAGE, |v| {
        v.to_str().expect("os string to be convertable")
    });
    Path::new(&*FACE_PATH).join(file_name).to_path_buf()
}

pub async fn try_image(opts: FnafOpts<'_>) -> Result<Vec<u8>, Box<dyn Error>> {
    let url = opts.custom_url.map_or(DEFAULT_IMAGE, |v| v);

    let mut image = if url.starts_with("http://") || url.starts_with("https://") {
        ImageReader::new(Cursor::new(reqwest::get(url).await?.bytes().await?))
            .with_guessed_format()?
            .decode()?
    } else {
        let path = get_local_image(Path::new(url));
        ImageReader::open(path)?.decode()?
    }
    .to_rgba8();

    add_text(&mut image, &FONT, opts);

    let encoder = webp::Encoder::from_rgba(&image, image.width(), image.height());
    let bytes = encoder
        .encode(
            env::var("FNAF_ENCODER_QUALITY")
                .ok()
                .and_then(|v| v.parse::<f32>().ok())
                .unwrap_or(70.0),
        )
        .to_vec();

    Ok(bytes)
}

fn add_text(image: &mut RgbaImage, font: &FontRef, opts: FnafOpts) {
    // defaults
    let naive_scale = PxScale::from(150.0);
    let default_text_element = TextElement {
        text_color: Rgba([255, 255, 255, 255]),
        outline_color: Rgba([0, 0, 0, 255]),
        font,
        content: "",
        scale: naive_scale,
    };
    let texts: Vec<&str> = [opts.top_text, opts.text, opts.bottom_text]
        .into_iter()
        .filter(|v| !v.is_empty())
        .collect();
    let divisions = texts.len();

    let bounding_box = {
        let dimensions = image.dimensions();
        (dimensions.0, dimensions.1 / divisions as u32)
    };

    texts.into_iter().enumerate().for_each(|(idx, text)| {
        if !text.is_empty() {
            let text_element = TextElement {
                content: text,
                scale: get_correct_scale(text, naive_scale, bounding_box, font),
                ..default_text_element
            };
            draw_text_with_border(
                image,
                &text_element,
                (text_element.scale.x * 0.015) as u8 * opts.outline_width,
                idx as u32,
                divisions as u32,
            );
        }
    })
}

fn get_correct_scale(
    text: &str,
    scale: PxScale,
    image_size: (u32, u32),
    font: &FontRef,
) -> PxScale {
    let size = text_size(scale, font, text);
    let sizes = [size.0, size.1];

    let largest_dim = sizes
        .iter()
        .enumerate()
        .max_by_key(|&(_, v)| v)
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    let scale = if largest_dim == 0 {
        scale.x * image_size.0 as f32 / size.0 as f32
    } else {
        scale.y * image_size.1 as f32 / size.1 as f32
    };

    let new_scale = PxScale::from(scale - MARGIN);
    let new_text_size = text_size(new_scale, font, text);

    match largest_dim {
        // todo: i sure wish there was a better way to do this
        0 => {
            if new_text_size.1 > image_size.1 {
                PxScale::from(new_scale.y * image_size.1 as f32 / new_text_size.1 as f32 - MARGIN)
            } else {
                new_scale
            }
        }
        1 => {
            if new_text_size.0 > image_size.0 {
                PxScale::from(new_scale.x * image_size.0 as f32 / new_text_size.0 as f32 - MARGIN)
            } else {
                new_scale
            }
        }
        _ => unreachable!(),
    }
}

/// draws `text_element` on section #`row_idx`
///
/// the image will get broken down into `row_total` sections,
/// each section is `canvas`.width wide and `canvas`.length / `row_total` tall
pub fn draw_text_with_border(
    canvas: &mut RgbaImage,
    text_element: &TextElement,
    outline_width: u8,
    row_idx: u32,
    row_total: u32,
) {
    let row_height = canvas.height() / row_total;
    let text_size = text_size(text_element.scale, text_element.font, text_element.content);

    // draw the raw text element
    let text_raw = draw_text(
        &RgbaImage::new(text_size.0, text_size.1),
        text_element.text_color,
        outline_width as i32,
        outline_width as i32,
        text_element.scale,
        text_element.font,
        text_element.content,
    );

    // a modified version of:
    // https://github.com/silvia-odwyer/gdl/blob/421c8df718ad32f66275d178edec56ec653caff9/crate/src/text.rs#L23
    let mut text_dilated: GrayImage = text_raw.convert();
    let mut text_to_draw = RgbaImage::new(text_size.0, text_size.1);
    dilate_mut(&mut text_dilated, Norm::LInf, outline_width);
    for x in 0..text_dilated.width() {
        for y in 0..text_dilated.height() {
            let pixval = 255 - text_dilated.get_pixel(x, y).0[0];
            if pixval != 255 {
                text_to_draw.put_pixel(x, y, text_element.outline_color);
            }
        }
    }

    overlay_mut(&mut text_to_draw, &text_raw, 0, 0);

    overlay_mut(
        canvas,
        &text_to_draw,
        canvas.width() / 2 - text_to_draw.width() / 2,
        row_height * row_idx + (row_height - text_to_draw.height()) / 2,
    );
}
