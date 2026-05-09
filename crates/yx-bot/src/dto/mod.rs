use std::sync::Arc;

pub struct QuoteAssets {
    pub font: ab_glyph::FontVec,
    pub watermark: image::RgbaImage,
    pub grain: image::RgbaImage,
}

impl std::fmt::Debug for QuoteAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QuoteAssets").finish()
    }
}

#[derive(Debug)]
pub struct AppData {
    pub db: yx_lib::db::DbManager,
    pub http: reqwest::Client,
    pub ai_api_key: String,
    pub ai_base_url: String,
    pub ai_model: String,
    pub ai_max_tokens: u32,
    pub emoji_cache: moka::future::Cache<String, image::RgbaImage>,
    pub quote_assets: Arc<QuoteAssets>,
}
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, AppData, Error>;
