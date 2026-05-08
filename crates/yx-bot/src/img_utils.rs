use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use std::error::Error;
use std::path::Path;

const BG_COLOR: Rgba<u8> = Rgba([10, 10, 10, 255]);
const TEXT_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);
const DIM_TEXT_COLOR: Rgba<u8> = Rgba([180, 180, 180, 255]);
const FOOTER_COLOR: Rgba<u8> = Rgba([100, 100, 100, 255]);

const CANVAS_WIDTH: u32 = 1200;
const CANVAS_HEIGHT: u32 = 630;
const LEFT_IMAGE_WIDTH: u32 = 550;
const RIGHT_SECTION_START: u32 = 550;
const RIGHT_SECTION_WIDTH: u32 = CANVAS_WIDTH - RIGHT_SECTION_START;
const PADDING: u32 = 80;
const WATERMARK_SIZE: u32 = 60;
const ICON_SIZE: u32 = 28;

pub async fn generate_quote_image(
    http: &reqwest::Client,
    avatar_url: &str,
    nickname: &str,
    username: &str,
    content: &str,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    let res_dir = if Path::new("crates/yx-bot/resources").exists() { "crates/yx-bot/resources" } else { "resources" };

    let font_path = Path::new(res_dir).join("font.ttf");
    let font_data = std::fs::read(font_path)?;
    let font = FontVec::try_from_vec(font_data)?;

    let logo_path = Path::new(res_dir).join("logo.png");
    let logo_img = image::open(logo_path)?;
    
    let mut watermark = logo_img.resize(WATERMARK_SIZE, WATERMARK_SIZE, image::imageops::FilterType::Lanczos3).to_rgba8();
    for p in watermark.pixels_mut() { p.0[3] = (p.0[3] as f32 * 0.4) as u8; }

    let icon = logo_img.resize(ICON_SIZE, ICON_SIZE, image::imageops::FilterType::Lanczos3).to_rgba8();

    let avatar_resp = http.get(avatar_url).send().await?.bytes().await?;
    let avatar_gray = image::load_from_memory(&avatar_resp)?.grayscale();

    // Iterative Text Scaling
    let mut content_size = 52.0;
    let mut wrapped_content = Vec::new();
    let mut line_height = 0;
    let max_text_w = RIGHT_SECTION_WIDTH - PADDING * 2;

    while content_size >= 20.0 {
        let scale = PxScale::from(content_size);
        wrapped_content = wrap_text_hard(content, &font, scale, max_text_w);
        line_height = (content_size * 1.35) as u32;
        let total_h = wrapped_content.len() as u32 * line_height + 120;
        if total_h <= 500 || content_size <= 20.0 { break; }
        content_size -= 4.0;
    }

    let content_scale = PxScale::from(content_size);
    let nick_scale = PxScale::from(28.0);
    let user_scale = PxScale::from(20.0);
    let footer_scale = PxScale::from(16.0);
    
    let mut img = RgbaImage::from_pixel(CANVAS_WIDTH, CANVAS_HEIGHT, BG_COLOR);

    // 1. Draw Cinematic Avatar
    let avatar_scaled = avatar_gray.resize_exact(LEFT_IMAGE_WIDTH, CANVAS_HEIGHT, image::imageops::FilterType::Lanczos3).to_rgba8();
    let mut avatar_faded = avatar_scaled.clone();
    for x in 0..LEFT_IMAGE_WIDTH {
        let alpha_factor = if x < LEFT_IMAGE_WIDTH / 2 { 1.0 } else { 1.0 - ((x - LEFT_IMAGE_WIDTH / 2) as f32 / (LEFT_IMAGE_WIDTH / 2) as f32) };
        for y in 0..CANVAS_HEIGHT {
            let p = avatar_faded.get_pixel_mut(x, y);
            p.0[3] = (p.0[3] as f32 * alpha_factor) as u8;
        }
    }
    image::imageops::overlay(&mut img, &avatar_faded, 0, 0);

    // 2. Draw Text Section (Centered)
    let total_text_h = wrapped_content.len() as u32 * line_height;
    let author_h = 100;
    let section_center_y = CANVAS_HEIGHT / 2;
    let text_start_y = section_center_y - (total_text_h + author_h) / 2;
    
    let mut current_y = text_start_y;
    for line in wrapped_content {
        let w = text_width(&line, &font, content_scale);
        let x = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (w / 2.0) as u32;
        draw_text_mut(&mut img, TEXT_COLOR, x as i32, current_y as i32, content_scale, &font, &line);
        current_y += line_height;
    }

    // 3. Draw Author Info
    current_y += 40;
    let nick_text = format!("- {}", nickname);
    let nw = text_width(&nick_text, &font, nick_scale);
    let nx = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (nw / 2.0) as u32;
    draw_text_mut(&mut img, TEXT_COLOR, nx as i32, current_y as i32, nick_scale, &font, &nick_text);
    
    current_y += 45;
    let user_text = format!("@{}", username);
    let uw = text_width(&user_text, &font, user_scale);
    let total_user_w = uw + (ICON_SIZE as f32) + 10.0;
    let ux_start = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (total_user_w / 2.0) as u32;
    
    image::imageops::overlay(&mut img, &icon, ux_start as i64, current_y as i64);
    draw_text_mut(&mut img, DIM_TEXT_COLOR, (ux_start + ICON_SIZE + 10) as i32, current_y as i32, user_scale, &font, &user_text);

    // 4. Draw Footer & Corner Watermark
    let wm_x = (CANVAS_WIDTH - WATERMARK_SIZE - 40) as i64;
    let wm_y = (CANVAS_HEIGHT - WATERMARK_SIZE - 40) as i64;
    image::imageops::overlay(&mut img, &watermark, wm_x, wm_y);

    let footer_text = "discord.gg/aqwcruel | © Cruel Quote System";
    let fw = text_width(footer_text, &font, footer_scale);
    draw_text_mut(&mut img, FOOTER_COLOR, (wm_x - fw as i64 - 15) as i32, (wm_y + (WATERMARK_SIZE / 2) as i64 - 8) as i32, footer_scale, &font, footer_text);

    let mut bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    img.write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(bytes)
}

fn wrap_text_hard<F: Font>(text: &str, font: &F, scale: PxScale, max_width: u32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current_line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        while !word.is_empty() {
            let test_line = if current_line.is_empty() { word.clone() } else { format!("{} {}", current_line, word) };
            if text_width(&test_line, font, scale) > max_width as f32 {
                if current_line.is_empty() {
                    let mut split_idx = word.len();
                    while split_idx > 0 && text_width(&word[..split_idx], font, scale) > max_width as f32 { split_idx -= 1; }
                    if split_idx == 0 { split_idx = 1; }
                    lines.push(word[..split_idx].to_string());
                    word = word[split_idx..].to_string();
                } else {
                    lines.push(current_line);
                    current_line = String::new();
                }
            } else {
                current_line = test_line;
                break;
            }
        }
    }
    if !current_line.is_empty() { lines.push(current_line); }
    lines
}

fn text_width<F: Font>(text: &str, font: &F, scale: PxScale) -> f32 {
    let scaled_font = font.as_scaled(scale);
    let mut width = 0.0;
    let mut last_glyph_id = None;
    for c in text.chars() {
        let glyph = scaled_font.scaled_glyph(c);
        if let Some(last) = last_glyph_id { width += scaled_font.kern(last, glyph.id); }
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
        let content = "Testing the final footer layout.";
        let _ = generate_quote_image(&http, avatar_url, nickname, username, content).await;
    }
}
