use crate::dto::{Context, Error};
use poise::serenity_prelude as serenity;
use std::time::{SystemTime, UNIX_EPOCH};

/// Show this help menu
#[poise::command(slash_command, prefix_command)]
pub async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"]
    #[autocomplete = "poise::builtins::autocomplete_command"]
    command: Option<String>,
) -> Result<(), Error> {
    poise::builtins::help(
        ctx,
        command.as_deref(),
        poise::builtins::HelpConfiguration {
            extra_text_at_bottom: "This is a demo bot scaffolded with serenity, poise, and sqlx.",
            ..Default::default()
        },
    )
    .await?;
    Ok(())
}

/// Ping the bot
#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Pong!").await?;
    Ok(())
}

/// Display information about a user
#[poise::command(slash_command, prefix_command)]
pub async fn userinfo(
    ctx: Context<'_>,
    #[description = "Selected user"] user: Option<serenity::User>,
) -> Result<(), Error> {
    let u = user.as_ref().unwrap_or(ctx.author());

    ctx.send(
        poise::CreateReply::default().embed(
            serenity::CreateEmbed::new()
                .title(format!("User Info - {}", u.name))
                .thumbnail(u.face())
                .field("ID", u.id.to_string(), true)
                .field("Bot", u.bot.to_string(), true)
                .field("Created At", u.created_at().to_string(), false)
                .color(0x00ff00),
        ),
    )
    .await?;

    Ok(())
}

/// Check your balance
#[poise::command(slash_command, prefix_command)]
pub async fn balance(ctx: Context<'_>) -> Result<(), Error> {
    let user = yx_lib::db::get_user(&ctx.data().db, ctx.author().id.into()).await?;

    ctx.send(
        poise::CreateReply::default().embed(
            serenity::CreateEmbed::new()
                .title(format!("{}'s Balance", ctx.author().name))
                .field("💰 Balance", format!("{} coins", user.points), true)
                .color(0xffd700),
        ),
    )
    .await?;

    Ok(())
}

/// Collect your daily reward
#[poise::command(slash_command, prefix_command)]
pub async fn daily(ctx: Context<'_>) -> Result<(), Error> {
    let user_id = ctx.author().id.get();
    let user = yx_lib::db::get_user(&ctx.data().db, user_id).await?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64;

    const ONE_DAY: i64 = 86400;

    if let Some(last_daily) = user.last_daily {
        if now - last_daily < ONE_DAY {
            let remaining = ONE_DAY - (now - last_daily);
            let hours = remaining / 3600;
            let minutes = (remaining % 3600) / 60;

            ctx.say(format!(
                "❌ You've already collected your daily reward! Try again in {}h {}m.",
                hours, minutes
            ))
            .await?;
            return Ok(());
        }
    }

    let reward = 100;
    sqlx::query("UPDATE users SET balance = balance + ?, last_daily = ? WHERE user_id = ?")
        .bind(reward)
        .bind(now)
        .bind(user_id as i64)
        .execute(&ctx.data().db)
        .await?;

    ctx.send(
        poise::CreateReply::default().embed(
            serenity::CreateEmbed::new()
                .title("🎁 Daily Reward")
                .description(format!("You've collected **{}** coins!", reward))
                .color(0x00ff00)
                .footer(serenity::CreateEmbedFooter::new(format!(
                    "New balance: {} coins",
                    user.points + reward
                ))),
        ),
    )
    .await?;

    Ok(())
}

/// Track a user's messages
#[poise::command(slash_command, prefix_command)]
pub async fn track(
    ctx: Context<'_>,
    #[description = "The user to track"] user: serenity::User,
    #[description = "Specific text to reply with"] reply: Option<String>,
    #[description = "Whether to repeat their message"] repeat: Option<bool>,
    #[description = "Whether to roast their message contextually"] roast: Option<bool>,
) -> Result<(), Error> {
    yx_lib::db::track_user(
        &ctx.data().db,
        user.id.get(),
        reply,
        repeat.unwrap_or(false),
        roast.unwrap_or(false),
    )
    .await?;
    ctx.say(format!("✅ Now tracking **{}**.", user.name))
        .await?;
    Ok(())
}

/// Stop tracking a user
#[poise::command(slash_command, prefix_command)]
pub async fn untrack(
    ctx: Context<'_>,
    #[description = "The user to stop tracking"] user: serenity::User,
) -> Result<(), Error> {
    yx_lib::db::untrack_user(&ctx.data().db, user.id.get()).await?;
    ctx.say(format!("❌ Stopped tracking **{}**.", user.name))
        .await?;
    Ok(())
}

/// View recent chat logs for a tracked user
#[poise::command(slash_command, prefix_command)]
pub async fn logs(
    ctx: Context<'_>,
    #[description = "The user whose logs to view"] user: serenity::User,
) -> Result<(), Error> {
    let user_id = user.id.get() as i64;
    let logs = sqlx::query(
        "SELECT content, timestamp FROM chat_logs WHERE user_id = ? ORDER BY timestamp DESC LIMIT 10"
    )
    .bind(user_id)
    .fetch_all(&ctx.data().db)
    .await?;

    if logs.is_empty() {
        ctx.say(format!("No logs found for **{}**.", user.name))
            .await?;
        return Ok(());
    }

    let mut response = format!("### Chat Logs for **{}**:\n", user.name);
    for log in logs {
        use sqlx::Row;
        let content: String = log.get("content");
        let timestamp: i64 = log.get("timestamp");
        response.push_str(&format!("- <t:{}:R> {}\n", timestamp, content));
    }

    ctx.say(response).await?;
    Ok(())
}

/// Create a premium quote image from a message
#[poise::command(
    context_menu_command = "Quote Message",
    slash_command,
    install_context = "Guild | User",
    interaction_context = "Guild | BotDm | PrivateChannel"
)]
pub async fn quote(
    ctx: Context<'_>,
    #[description = "The message to quote"] msg: serenity::Message,
) -> Result<(), Error> {
    ctx.defer().await?;

    let author = &msg.author;
    let guild_id = msg.guild_id;

    // Get nickname if in a guild
    let display_name = if let Some(gid) = guild_id {
        let member_res = gid.member(ctx, author.id).await;
        if let Ok(member) = member_res {
            member.display_name().to_string()
        } else {
            println!("Cache member fetch failed: {:?}", member_res.err());
            let http_member_res = ctx.http().get_member(gid, author.id).await;
            if let Ok(member) = http_member_res {
                member.display_name().to_string()
            } else {
                println!("HTTP member fetch failed: {:?}", http_member_res.err());
                author
                    .global_name
                    .clone()
                    .unwrap_or_else(|| author.name.clone())
            }
        }
    } else {
        author
            .global_name
            .clone()
            .unwrap_or_else(|| author.name.clone())
    };

    let mut avatar_url = author.face();
    if avatar_url.contains('?') {
        avatar_url = format!("{}&size=4096", avatar_url);
    } else {
        avatar_url = format!("{}?size=4096", avatar_url);
    }
    let content = msg.content_safe(ctx);

    let image_bytes = crate::img_utils::generate_quote_image(
        &ctx.data().http,
        &avatar_url,
        &display_name,
        &author.name,
        &content,
    )
    .await?;

    let filename = format!(
        "quote_{}{}.png",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    );
    ctx.send(
        poise::CreateReply::default()
            .attachment(serenity::CreateAttachment::bytes(image_bytes, filename)),
    )
    .await?;

    Ok(())
}
