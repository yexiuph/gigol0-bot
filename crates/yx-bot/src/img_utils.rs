use ab_glyph::{Font, FontVec, PxScale, ScaleFont};
use futures::future::join_all;
use image::{Rgba, RgbaImage};
use imageproc::drawing::draw_text_mut;
use regex::Regex;
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

#[derive(Clone)]
enum RichSegment {
    Text(String),
    Emoji(RgbaImage),
}

type RichLine = Vec<RichSegment>;

const BG_COLOR: Rgba<u8> = Rgba([0, 0, 0, 255]);
const TEXT_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]);
const DIM_TEXT_COLOR: Rgba<u8> = Rgba([180, 180, 180, 255]);
const FOOTER_COLOR: Rgba<u8> = Rgba([100, 100, 100, 255]);

const TARGET_WIDTH: u32 = 1200;
const TARGET_HEIGHT: u32 = 630;
const SCALE: u32 = 2;
const CANVAS_WIDTH: u32 = TARGET_WIDTH * SCALE;
const CANVAS_HEIGHT: u32 = TARGET_HEIGHT * SCALE;

const LEFT_IMAGE_WIDTH: u32 = 550 * SCALE;
const RIGHT_SECTION_START: u32 = 550 * SCALE;
const RIGHT_SECTION_WIDTH: u32 = CANVAS_WIDTH - RIGHT_SECTION_START;
const PADDING: u32 = 80 * SCALE;
const WATERMARK_SIZE: u32 = 60 * SCALE;
const ICON_SIZE: u32 = 28 * SCALE;

pub async fn generate_quote_image(
    http: &reqwest::Client,
    cache: &moka::future::Cache<String, RgbaImage>,
    avatar_url: &str,
    nickname: &str,
    username: &str,
    content: &str,
) -> Result<Vec<u8>, Box<dyn Error + Send + Sync>> {
    // 0. Parse Rich Content & Identify Fetch Targets
    let custom_emoji_regex = Regex::new(r"<(a?):(\w+):(\d+)>").unwrap();
    let combined_regex = Regex::new(
        r"<(?:a?):(?:\w+):(?:\d+)>|[\x{1F300}-\x{1F9FF}\x{2600}-\x{26FF}\x{2700}-\x{27BF}]",
    )
    .unwrap();

    #[derive(Clone)]
    enum FetchSegment {
        Text(String),
        EmojiUrl(String, String), // (URL, OriginalMatch)
    }

    let mut fetch_plan = Vec::new();
    let mut last_idx = 0;
    let mut unique_urls = std::collections::HashSet::new();

    for mat in combined_regex.find_iter(content) {
        let before = &content[last_idx..mat.start()];
        if !before.is_empty() {
            fetch_plan.push(FetchSegment::Text(before.to_string()));
        }

        let m_str = mat.as_str();
        let url = if let Some(caps) = custom_emoji_regex.captures(m_str) {
            let id = caps.get(3).unwrap().as_str();
            format!("https://cdn.discordapp.com/emojis/{}.png?size=128", id)
        } else {
            let hex = m_str
                .chars()
                .filter(|&c| c != '\u{FE0F}')
                .map(|c| format!("{:x}", c as u32))
                .collect::<Vec<_>>()
                .join("-");
            format!(
                "https://cdn.jsdelivr.net/gh/twitter/twemoji@latest/assets/72x72/{}.png",
                hex
            )
        };

        unique_urls.insert(url.clone());
        fetch_plan.push(FetchSegment::EmojiUrl(url, m_str.to_string()));
        last_idx = mat.end();
    }

    let last_part = &content[last_idx..];
    if !last_part.is_empty() {
        fetch_plan.push(FetchSegment::Text(last_part.to_string()));
    }
    if fetch_plan.is_empty() && !content.is_empty() {
        fetch_plan.push(FetchSegment::Text(content.to_string()));
    }

    // 1. Parallel Fetching with Cache
    let emoji_fetches = unique_urls.into_iter().map(|url| {
        let http = http.clone();
        let cache = cache.clone();
        async move {
            if let Some(img) = cache.get(&url).await {
                return (url, Some(img));
            }
            if let Ok(resp) = http.get(&url).send().await {
                if let Ok(bytes) = resp.bytes().await {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let rgba = img.to_rgba8();
                        cache.insert(url.clone(), rgba.clone()).await;
                        return (url, Some(rgba));
                    }
                }
            }
            (url, None)
        }
    });

    let avatar_cache_key = format!("avatar:{}", avatar_url);
    let avatar_fetch = {
        let http = http.clone();
        let cache = cache.clone();
        let avatar_url = avatar_url.to_string();
        async move {
            if let Some(img) = cache.get(&avatar_cache_key).await {
                return Some(img);
            }
            if let Ok(resp) = http.get(&avatar_url).send().await {
                if let Ok(bytes) = resp.bytes().await {
                    if let Ok(img) = image::load_from_memory(&bytes) {
                        let scaled = img.resize_to_fill(
                            LEFT_IMAGE_WIDTH,
                            CANVAS_HEIGHT,
                            image::imageops::FilterType::CatmullRom,
                        );
                        let gray = scaled.grayscale().to_rgba8();
                        cache.insert(avatar_cache_key, gray.clone()).await;
                        return Some(gray);
                    }
                }
            }
            None
        }
    };

    let (emoji_results, avatar_res) = futures::join!(join_all(emoji_fetches), avatar_fetch);
    let emoji_map: HashMap<String, RgbaImage> = emoji_results
        .into_iter()
        .filter_map(|(url, img)| img.map(|i| (url, i)))
        .collect();

    let mut segments = Vec::new();
    for item in fetch_plan {
        match item {
            FetchSegment::Text(t) => segments.push(RichSegment::Text(t)),
            FetchSegment::EmojiUrl(url, m) => {
                if let Some(img) = emoji_map.get(&url) {
                    segments.push(RichSegment::Emoji(img.clone()));
                } else {
                    segments.push(RichSegment::Text(m));
                }
            }
        }
    }
    let res_dir = if Path::new("crates/yx-bot/resources").exists() {
        "crates/yx-bot/resources"
    } else {
        "resources"
    };
    let font_path = Path::new(res_dir).join("font.ttf");
    let font_data = std::fs::read(font_path)?;
    let font = FontVec::try_from_vec(font_data)?;

    let logo_path = Path::new(res_dir).join("logo.png");
    let logo_img = image::open(logo_path)?;
    let mut watermark = logo_img
        .resize(
            WATERMARK_SIZE,
            WATERMARK_SIZE,
            image::imageops::FilterType::CatmullRom,
        )
        .to_rgba8();
    for p in watermark.pixels_mut() {
        p.0[3] = (p.0[3] as f32 * 0.4) as u8;
    }
    let icon = logo_img
        .resize(
            ICON_SIZE,
            ICON_SIZE,
            image::imageops::FilterType::CatmullRom,
        )
        .to_rgba8();

    let mut avatar_gray =
        avatar_res.unwrap_or_else(|| RgbaImage::new(LEFT_IMAGE_WIDTH, CANVAS_HEIGHT));

    let mut img = RgbaImage::from_pixel(CANVAS_WIDTH, CANVAS_HEIGHT, BG_COLOR);

    // 1. Draw Cinematic Avatar with Natural Smooth Vignette
    for x in 0..LEFT_IMAGE_WIDTH {
        let fade_start = (LEFT_IMAGE_WIDTH as f32 * 0.6) as u32;
        let alpha_factor = if x < fade_start {
            1.0
        } else {
            let progress = (x - fade_start) as f32 / (LEFT_IMAGE_WIDTH - fade_start) as f32;
            1.0 - progress
        };
        for y in 0..CANVAS_HEIGHT {
            let p = avatar_gray.get_pixel_mut(x, y);
            p.0[3] = (p.0[3] as f32 * alpha_factor) as u8;
        }
    }
    image::imageops::overlay(&mut img, &avatar_gray, 0, 0);

    // 2. Iterative Font Scaling
    let mut content_size = 70.0 * SCALE as f32; // Start with a large cinematic size
    let mut wrapped_content = Vec::new();
    let mut final_line_height = 0.0;
    let max_text_h = (CANVAS_HEIGHT as f32) * 0.75; // Use 75% of height for text

    while content_size >= 25.0 * SCALE as f32 {
        let test_scale = PxScale::from(content_size);
        let test_line_height = content_size * 1.5;
        wrapped_content = wrap_rich_text(
            &segments,
            &font,
            test_scale,
            (RIGHT_SECTION_WIDTH - PADDING * 2) as f32,
        );

        let total_h = (wrapped_content.len() as f32 * test_line_height) + (100.0 * SCALE as f32);
        if total_h <= max_text_h {
            final_line_height = test_line_height;
            break;
        }
        content_size -= 4.0;
        final_line_height = test_line_height;
    }

    let content_scale = PxScale::from(content_size);
    let line_height = final_line_height;
    let final_emoji_size = content_size as u32;

    // Resize emojis to final scale
    let mut scaled_wrapped = Vec::new();
    for line in wrapped_content {
        let mut scaled_line = Vec::new();
        for segment in line {
            match segment {
                RichSegment::Text(t) => scaled_line.push(RichSegment::Text(t)),
                RichSegment::Emoji(img) => {
                    let scaled_emoji = image::imageops::resize(
                        &img,
                        final_emoji_size,
                        final_emoji_size,
                        image::imageops::FilterType::CatmullRom,
                    );
                    scaled_line.push(RichSegment::Emoji(scaled_emoji));
                }
            }
        }
        scaled_wrapped.push(scaled_line);
    }

    let total_text_height = (scaled_wrapped.len() as f32 * line_height) + (100.0 * SCALE as f32);
    let mut current_y = (CANVAS_HEIGHT / 2) as f32 - (total_text_height / 2.0);

    for line in scaled_wrapped {
        let w = rich_line_width(&line, &font, content_scale);
        let mut current_x = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (w / 2.0) as u32;

        for segment in line {
            match segment {
                RichSegment::Text(t) => {
                    draw_text_mut(
                        &mut img,
                        TEXT_COLOR,
                        current_x as i32,
                        current_y as i32,
                        content_scale,
                        &font,
                        &t,
                    );
                    current_x += text_width(&t, &font, content_scale) as u32;
                }
                RichSegment::Emoji(e) => {
                    let emoji_y = current_y + (line_height / 2.0)
                        - (final_emoji_size as f32 / 2.0)
                        - (5.0 * SCALE as f32);
                    image::imageops::overlay(&mut img, &e, current_x as i64, emoji_y as i64);
                    current_x += final_emoji_size + (4 * SCALE);
                }
            }
        }
        current_y += line_height;
    }

    // 3. Draw Author Info
    current_y += 40.0 * SCALE as f32;
    let nick_scale = PxScale::from(28.0 * SCALE as f32);
    let user_scale = PxScale::from(20.0 * SCALE as f32);
    let footer_scale = PxScale::from(16.0 * SCALE as f32);

    let nick_text = format!("- {}", nickname);
    let nw = text_width(&nick_text, &font, nick_scale);
    let nx = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (nw / 2.0) as u32;
    draw_text_mut(
        &mut img,
        TEXT_COLOR,
        nx as i32,
        current_y as i32,
        nick_scale,
        &font,
        &nick_text,
    );

    current_y += 45.0 * SCALE as f32;
    let user_text = format!("@{}", username);
    let uw = text_width(&user_text, &font, user_scale);
    let ux_start = RIGHT_SECTION_START + (RIGHT_SECTION_WIDTH / 2) - (uw / 2.0) as u32;

    draw_text_mut(
        &mut img,
        DIM_TEXT_COLOR,
        ux_start as i32,
        current_y as i32,
        user_scale,
        &font,
        &user_text,
    );

    // 4. Draw Footer & Corner Watermark
    let wm_x = (CANVAS_WIDTH - WATERMARK_SIZE - 40 * SCALE) as i64;
    let wm_y = (CANVAS_HEIGHT - WATERMARK_SIZE - 40 * SCALE) as i64;
    image::imageops::overlay(&mut img, &watermark, wm_x, wm_y);

    let footer_text_env = std::env::var("QUOTE_FOOTER")
        .unwrap_or_else(|_| "discord.gg/aqwcruel | © Cruel Quote System".to_string());
    let footer_text = &footer_text_env;
    let fw = text_width(footer_text, &font, footer_scale);
    draw_text_mut(
        &mut img,
        FOOTER_COLOR,
        (wm_x - fw as i64 - 15 * SCALE as i64) as i32,
        (wm_y + (WATERMARK_SIZE / 2) as i64 - 8 * SCALE as i64) as i32,
        footer_scale,
        &font,
        footer_text,
    );

    // 5. Final Downsample (2x -> 1x)
    let final_img = image::imageops::resize(
        &img,
        TARGET_WIDTH,
        TARGET_HEIGHT,
        image::imageops::FilterType::Lanczos3,
    );

    let mut bytes: Vec<u8> = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut bytes);
    final_img.write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(bytes)
}

fn rich_line_width<F: Font>(line: &RichLine, font: &F, scale: PxScale) -> f32 {
    let mut w = 0.0;
    for segment in line {
        match segment {
            RichSegment::Text(t) => w += text_width(t, font, scale),
            RichSegment::Emoji(e) => w += (e.width() as f32) + (4.0 * SCALE as f32),
        }
    }
    w
}

fn wrap_rich_text<F: Font>(
    segments: &[RichSegment],
    font: &F,
    scale: PxScale,
    max_width: f32,
) -> Vec<RichLine> {
    let mut lines = Vec::new();
    let mut current_line = RichLine::new();
    let mut current_width = 0.0;
    let emoji_w = scale.y + (4.0 * SCALE as f32);

    for segment in segments {
        match segment {
            RichSegment::Text(t) => {
                for word in t.split_whitespace() {
                    let word_with_space = if current_line.is_empty() {
                        word.to_string()
                    } else {
                        format!(" {}", word)
                    };
                    let word_w = text_width(&word_with_space, font, scale);

                    if current_width + word_w > max_width {
                        if !current_line.is_empty() {
                            lines.push(current_line);
                            current_line = RichLine::new();
                            current_width = 0.0;
                        }
                        let clean_word = word.to_string();
                        current_line.push(RichSegment::Text(clean_word.clone()));
                        current_width = text_width(&clean_word, font, scale);
                    } else {
                        if let Some(RichSegment::Text(last_t)) = current_line.last_mut() {
                            last_t.push_str(&word_with_space);
                        } else {
                            current_line.push(RichSegment::Text(word_with_space));
                        }
                        current_width += word_w;
                    }
                }
            }
            RichSegment::Emoji(e) => {
                if current_width + emoji_w > max_width {
                    if !current_line.is_empty() {
                        lines.push(current_line);
                    }
                    current_line = vec![RichSegment::Emoji(e.clone())];
                    current_width = emoji_w;
                } else {
                    current_line.push(RichSegment::Emoji(e.clone()));
                    current_width += emoji_w;
                }
            }
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
