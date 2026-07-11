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
const MARGIN: f32 = 2.0;

#[derive(Clone, Copy)]
pub struct TextElement<'a> {
    content: &'a str,
    scale: PxScale,
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
    let (width, height) = image.dimensions();
    let naive_scale = PxScale::from(150.0);
    let mut texts: Vec<TextElement> = Vec::new();

    // TODO: foreach through the opts.*_text[] may be cleaner
    if !opts.top_text.is_empty() {
        dbg!(opts.top_text);
        texts.push(TextElement {
            content: opts.top_text,
            scale: get_correct_scale(opts.top_text, naive_scale, (width, height), font),
        });
    }
    if !opts.text.is_empty() {
        dbg!(opts.text);
        texts.push(TextElement {
            content: opts.text,
            scale: get_correct_scale(opts.text, naive_scale, (width, height), font),
        });
    }
    if !opts.bottom_text.is_empty() {
        dbg!(opts.bottom_text);
        texts.push(TextElement {
            content: opts.bottom_text,
            scale: get_correct_scale(opts.bottom_text, naive_scale, (width, height), font),
        });
    }

    texts.iter().enumerate().for_each(|(idx, text)| {
        draw_text_with_border(
            image,
            Rgba([255, 255, 255, 255]),
            *text,
            font,
            Rgba([0, 0, 0, 255]),
            (text.scale.x * 0.015) as u8 * opts.outline_width,
            idx,
            texts.len(),
        );
    });
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
    PxScale::from(scale - MARGIN)
}

// a modified version of:
// https://github.com/silvia-odwyer/gdl/blob/421c8df718ad32f66275d178edec56ec653caff9/crate/src/text.rs#L23
#[allow(clippy::too_many_arguments)]
pub fn draw_text_with_border(
    canvas: &mut RgbaImage,
    color: Rgba<u8>,
    text_element: TextElement,
    font: &FontRef,
    outline_color: Rgba<u8>,
    outline_width: u8,
    row_idx: usize,
    rows_total: usize,
) {
    let scale = text_element.scale;
    let text = text_element.content;
    if text.trim() == "" {
        return;
    }

    // intialize text width / height including the needed space of the outlines
    let (text_width, text_height, canvas_width, canvas_height) = {
        let text_bbox = text_size(scale, font, text);
        (
            text_bbox.0 as f32 + (outline_width * 2) as f32,
            text_bbox.1 as f32 + (outline_width * 2) as f32,
            canvas.width() as f32,
            canvas.height() as f32,
        )
    };
    let row_height = canvas_height / rows_total as f32;
    let project_scale = f32::min(
        canvas_width / text_width,
        canvas_height / text_height / rows_total as f32,
    );
    let project_op = Projection::scale(project_scale, project_scale);

    // draw the text element
    let text_raw = draw_text(
        &RgbaImage::new(text_width as u32, text_height as u32),
        color,
        outline_width as i32,
        outline_width as i32,
        scale,
        font,
        text,
    );

    // dilate to outline_width -> color it with outline_color -> blur for aa effect
    let mut text_dilated: GrayImage = text_raw.convert();
    let mut text_to_draw = RgbaImage::new(text_width as u32, text_height as u32);
    dilate_mut(&mut text_dilated, Norm::LInf, outline_width);
    for x in 0..text_dilated.width() {
        for y in 0..text_dilated.height() {
            let pixval = 255 - text_dilated.get_pixel(x, y).0[0];
            if pixval != 255 {
                text_to_draw.put_pixel(x, y, outline_color);
            }
        }
    }
    // text_to_draw = gaussian_blur_f32(&text_to_draw, 0.7);

    // draw actual text on top of outline
    overlay_mut(&mut text_to_draw, &text_raw, 0, 0);

    // scale text object and overlay on canvas
    let text_transformed = warp(
        &text_to_draw,
        project_op,
        Interpolation::Bicubic,
        Border::Constant(Rgba([0, 0, 0, 0])),
    );
    overlay_mut(
        canvas,
        &text_transformed,
        ((canvas_width - text_transformed.width() as f32) * project_scale / 2.0) as u32,
        (row_height * row_idx as f32
            + (row_height - text_transformed.height() as f32 * project_scale) / 2.0) as u32,
    );
}
