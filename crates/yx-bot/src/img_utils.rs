use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use std::error::Error;
use std::path::Path;

const BG_COLOR: Rgba<u8> = Rgba([15, 15, 15, 255]);
const TEXT_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);
const DIM_TEXT_COLOR: Rgba<u8> = Rgba([150, 150, 150, 255]);
const DIVIDER_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);
const FOOTER_COLOR: Rgba<u8> = Rgba([100, 100, 100, 255]);

const CANVAS_WIDTH: u32 = 900;
const AVATAR_SIZE: u32 = 128;
const PADDING: u32 = 40;
const LEFT_SECTION_WIDTH: u32 = 220;
const TEXT_START_X: u32 = LEFT_SECTION_WIDTH + 20;
const MAX_TEXT_WIDTH: u32 = CANVAS_WIDTH - TEXT_START_X - PADDING;
const WATERMARK_SIZE: u32 = 300;

pub async fn generate_quote_image(
    http: &reqwest::Client,
    avatar_url: &str,
    nickname: &str,
    username: &str,
    content: &str,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    // Determine resources path
    let res_dir = if Path::new("crates/yx-bot/resources").exists() {
        "crates/yx-bot/resources"
    } else {
        "resources"
    };

    // Load font
    let font_path = Path::new(res_dir).join("font.ttf");
    let font_data = std::fs::read(font_path)?;
    let font = FontVec::try_from_vec(font_data)?;

    // Load logo as watermark
    let logo_path = Path::new(res_dir).join("logo.png");
    let mut watermark = image::open(logo_path)?;
    watermark = watermark.resize(
        WATERMARK_SIZE,
        WATERMARK_SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    let mut watermark_rgba = watermark.to_rgba8();

    // Lower opacity of watermark
    for pixel in watermark_rgba.pixels_mut() {
        pixel.0[3] = (pixel.0[3] as f32 * 0.15) as u8; // 15% opacity
    }

    // Fetch and process avatar
    let avatar_resp = http.get(avatar_url).send().await?.bytes().await?;
    let mut avatar = image::load_from_memory(&avatar_resp)?;
    avatar = avatar.resize_exact(
        AVATAR_SIZE,
        AVATAR_SIZE,
        image::imageops::FilterType::Lanczos3,
    );
    let avatar = circular_crop(avatar);

    // Prepare text wrapping
    let nick_scale = PxScale::from(24.0);
    let user_scale = PxScale::from(18.0);
    let content_scale = PxScale::from(28.0);
    let footer_scale = PxScale::from(16.0);

    let wrapped_content = wrap_text(content, &font, content_scale, MAX_TEXT_WIDTH);
    let line_height = 36;
    let total_text_height = wrapped_content.len() as u32 * line_height;

    // Calculate total height needed
    let left_height = AVATAR_SIZE + 60;
    let canvas_height =
        std::cmp::max(left_height + PADDING * 2, total_text_height + PADDING * 2) + 100; // Extra space for footer

    let mut img = RgbaImage::from_pixel(CANVAS_WIDTH, canvas_height, BG_COLOR);

    // Draw Watermark (Behind text)
    let wm_x = (TEXT_START_X + (MAX_TEXT_WIDTH / 2) - (WATERMARK_SIZE / 2)) as i64;
    let wm_y = (canvas_height / 2 - WATERMARK_SIZE / 2) as i64;
    image::imageops::overlay(&mut img, &watermark_rgba, wm_x, wm_y);

    // Draw Avatar (Centered in left section)
    let avatar_x = (LEFT_SECTION_WIDTH / 2 - AVATAR_SIZE / 2) as i64;
    let avatar_y = (canvas_height / 2 - left_height / 2 - 30) as i64;
    image::imageops::overlay(&mut img, &avatar, avatar_x, avatar_y);

    // Draw Nickname (Under Avatar)
    let nick_w = text_width(nickname, &font, nick_scale);
    let nick_x = (LEFT_SECTION_WIDTH as f32 / 2.0 - nick_w / 2.0) as i32;
    draw_text_mut(
        &mut img,
        TEXT_COLOR,
        nick_x,
        (avatar_y + AVATAR_SIZE as i64 + 10) as i32,
        nick_scale,
        &font,
        nickname,
    );

    // Draw Username (Under Nickname)
    let formatted_user = format!("@{}", username);
    let user_w = text_width(&formatted_user, &font, user_scale);
    let user_x = (LEFT_SECTION_WIDTH as f32 / 2.0 - user_w / 2.0) as i32;
    draw_text_mut(
        &mut img,
        DIM_TEXT_COLOR,
        user_x,
        (avatar_y + AVATAR_SIZE as i64 + 38) as i32,
        user_scale,
        &font,
        &formatted_user,
    );

    // Draw Divider
    let divider_x = LEFT_SECTION_WIDTH as i32;
    draw_filled_rect_mut(
        &mut img,
        Rect::at(divider_x, PADDING as i32).of_size(2, canvas_height - PADDING * 2 - 40),
        DIVIDER_COLOR,
    );

    // Draw Content
    let content_start_y = (canvas_height / 2 - total_text_height / 2 - 20) as u32;
    let mut current_y = content_start_y;
    for line in wrapped_content {
        draw_text_mut(
            &mut img,
            TEXT_COLOR,
            TEXT_START_X as i32,
            current_y as i32,
            content_scale,
            &font,
            &line,
        );
        current_y += line_height;
    }

    // Draw Footer Text
    let footer_text =
        "Stop stealing our stuffs, go join us! discord.gg/aqwcruel and add referrer : ainsworth";
    let footer_w = text_width(footer_text, &font, footer_scale);
    let footer_x = (CANVAS_WIDTH as f32 / 2.0 - footer_w / 2.0) as i32;
    draw_text_mut(
        &mut img,
        FOOTER_COLOR,
        footer_x,
        (canvas_height - 30) as i32,
        footer_scale,
        &font,
        footer_text,
    );

    let mut bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)?;

    Ok(bytes)
}

fn circular_crop(img: DynamicImage) -> RgbaImage {
    let (width, height) = img.dimensions();
    let mut output = RgbaImage::new(width, height);
    let radius = (width / 2) as f32;
    let center = (radius, (height / 2) as f32);

    for (x, y, pixel) in img.to_rgba8().enumerate_pixels() {
        let dx = x as f32 - center.0 + 0.5;
        let dy = y as f32 - center.1 + 0.5;
        if dx * dx + dy * dy <= radius * radius {
            output.put_pixel(x, y, *pixel);
        }
    }
    output
}

fn wrap_text<F: Font>(text: &str, font: &F, scale: PxScale, max_width: u32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        let test_line = if current_line.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current_line, word)
        };

        let width = text_width(&test_line, font, scale);
        if width > max_width as f32 && !current_line.is_empty() {
            lines.push(current_line);
            current_line = word.to_string();
        } else {
            current_line = test_line;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    lines
}

fn text_width<F: Font>(text: &str, font: &F, scale: PxScale) -> f32 {
    let scaled_font = font.as_scaled(scale);
    let mut width = 0.0;
    let mut last_glyph_id = None;
    for c in text.chars() {
        let glyph = scaled_font.scaled_glyph(c);
        if let Some(last) = last_glyph_id {
            width += scaled_font.kern(last, glyph.id);
        }
        width += scaled_font.h_advance(glyph.id);
        last_glyph_id = Some(glyph.id);
    }
    width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quote_gen() {
        let http = reqwest::Client::new();
        let avatar_url = "https://cdn.discordapp.com/embed/avatars/0.png";
        let nickname = "Ainsworth";
        let username = "yexiuph";
        let content = "SYBAU IS THE BEST! I LOVE SYBAU! SYBAU! I LOVE SYBAU! SYBAU! I LOVE SYBAU! SYBAU! I LOVE SYBAU! SYBAU! I LOVE SYBAU! ";

        println!("Current directory: {:?}", std::env::current_dir());
        let result = generate_quote_image(&http, avatar_url, nickname, username, content).await;
        match result {
            Ok(bytes) => {
                std::fs::write("test_quote_footer.png", bytes).expect("Failed to write test image");
                println!("Test quote image generated: test_quote_footer.png");
            }
            Err(e) => panic!("Failed to generate quote image: {:?}", e),
        }
    }
}
