pub mod dto;
mod commands;
mod img_utils;

use dto::AppData;
use yx_lib::db;
use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Load environment variables
    dotenvy::dotenv().ok();

    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("golo_bot=info,serenity=info,poise=info")))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize database
    let db = db::setup_database().await?;
    info!("Database initialized and migrations applied.");

    // Setup poise framework
    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                commands::quote(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("!".into()),
                ..Default::default()
            },
            on_error: |error| {
                Box::pin(async move {
                    error!("Error within framework: {:?}", error);
                })
            },
            event_handler: |ctx, event, _framework, data| {
                Box::pin(async move {
                    if let serenity::FullEvent::Message { new_message } = event {
                        if new_message.author.bot {
                            return Ok(());
                        }

                        info!(
                            "Received message from {}: {}",
                            new_message.author.name, new_message.content
                        );

                        if let Ok(Some(tracking)) =
                            db::get_tracked_user(&data.db, new_message.author.id.get()).await
                        {
                            // Ensure user exists in users table (foreign key requirement)
                            let _ = db::get_user(&data.db, new_message.author.id.get()).await;
                            info!(
                                "Tracking active for user {}. repeat_message: {}, reply_text: {:?}",
                                new_message.author.name,
                                tracking.repeat_message,
                                tracking.reply_text
                            );
                            // Log the chat
                            if let Err(e) = db::log_chat(
                                &data.db,
                                new_message.author.id.get(),
                                &new_message.content,
                            )
                            .await
                            {
                                error!("Failed to log chat: {:?}", e);
                            }

                            // Reply if needed
                            if tracking.roast_mode && !data.ai_api_key.is_empty() {
                                let prompt = format!(
                                    "You are a witty, sarcastic Filipino. You speak fluent English. Roast this message from {}: \"{}\". Keep it short, savage, and brutal.",
                                    new_message.author.name, new_message.content
                                );

                                let body = serde_json::json!({
                                    "model": data.ai_model,
                                    "messages": [{"role": "user", "content": prompt}],
                                    "max_tokens": data.ai_max_tokens
                                });

                                info!("Sending AI Roast request to {} with prompt: {}", data.ai_base_url, prompt);

                                let response = data.http.post(format!("{}/chat/completions", data.ai_base_url))
                                    .header("Authorization", format!("Bearer {}", data.ai_api_key))
                                    .json(&body)
                                    .send()
                                    .await;

                                match response {
                                    Ok(res) => {
                                        if let Ok(json) = res.json::<serde_json::Value>().await {
                                            if let Some(roast) = json["choices"][0]["message"]["content"].as_str() {
                                                info!("AI Roast Response: {}", roast);
                                                if let Some(usage) = json.get("usage") {
                                                    info!("AI Usage: {:?}", usage);
                                                }
                                                if let Err(e) = new_message.reply(ctx, roast).await {
                                                    error!("Failed to send roast: {:?}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => error!("Failed to call AI API: {:?}", e),
                                }
                            } else if tracking.repeat_message {
                                let sanitized_msg = &new_message.content.trim().to_string();
                                if let Err(e) = new_message.reply(ctx, sanitized_msg).await {
                                    error!("Failed to repeat message: {:?}", e);
                                }
                            } else if let Some(reply_text) = &tracking.reply_text {
                                if let Err(e) = new_message.reply(ctx, reply_text).await {
                                    error!("Failed to send tracking reply: {:?}", e);
                                }
                            }
                        }
                    }
                    Ok(())
                })
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                info!("Bot is ready as {}", _ready.user.name);
                
                // Register globally (takes up to 1 hour to update, but required for User App)
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                
                let ai_api_key = env::var("AI_API_KEY").unwrap_or_default();
                let ai_base_url = env::var("AI_BASE_URL").unwrap_or_else(|_| "https://api.groq.com/openai/v1".to_string());
                let ai_model = env::var("AI_MODEL").unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string());
                let ai_max_tokens = env::var("AI_MAX_TOKENS").unwrap_or_else(|_| "100".to_string()).parse().unwrap_or(100);
                
                let emoji_cache = moka::future::Cache::builder()
                    .max_capacity(1000)
                    .time_to_idle(std::time::Duration::from_secs(60 * 60 * 24)) // 24h cache
                    .build();

                Ok(AppData { 
                    db, 
                    http: reqwest::Client::new(),
                    ai_api_key,
                    ai_base_url,
                    ai_model,
                    ai_max_tokens,
                    emoji_cache,
                })
            })
        })
        .build();

    let token = env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await?;

    info!("Starting bot...");
    client.start().await?;

    Ok(())
}
