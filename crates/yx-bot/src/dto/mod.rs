#[derive(Debug)]
pub struct AppData {
    pub db: yx_lib::db::DbManager,
    pub http: reqwest::Client,
    pub ai_api_key: String,
    pub ai_base_url: String,
    pub ai_model: String,
    pub ai_max_tokens: u32,
    pub emoji_cache: moka::future::Cache<String, image::RgbaImage>,
}
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, AppData, Error>;
