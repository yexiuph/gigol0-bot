pub mod models;
pub mod repositories;

use sqlx::{FromRow, Pool, Sqlite};

pub type DbManager = Pool<Sqlite>;

pub async fn setup_database() -> Result<DbManager, sqlx::Error> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    // Create the database file if it doesn't exist
    if !std::path::Path::new("database.db").exists() {
        std::fs::File::create("database.db").expect("Couldn't create database file");
    }

    let pool = Pool::<Sqlite>::connect(&database_url).await?;

    // Run migrations
    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(pool)
}

#[derive(FromRow, Debug, Clone)]
pub struct UserTracking {
    pub user_id: i64,
    pub reply_text: Option<String>,
    pub repeat_message: bool,
    pub roast_mode: bool,
}

pub async fn get_user(
    pool: &DbManager,
    user_id: u64,
) -> Result<models::user::UserProfile, sqlx::Error> {
    let user_id = user_id as i64;
    let user = sqlx::query_as::<_, models::user::UserProfile>(
        "SELECT user_id, points, last_daily FROM users WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    if let Some(user) = user {
        Ok(user)
    } else {
        // Create user if they don't exist
        sqlx::query("INSERT INTO users (user_id, points) VALUES (?, ?)")
            .bind(user_id)
            .bind(0)
            .execute(pool)
            .await?;

        Ok(models::user::UserProfile {
            user_id,
            points: 0,
            last_daily: None,
        })
    }
}

pub async fn add_balance(pool: &DbManager, user_id: u64, amount: i64) -> Result<(), sqlx::Error> {
    let user_id = user_id as i64;
    sqlx::query("UPDATE users SET points = points + ? WHERE user_id = ?")
        .bind(amount)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn track_user(
    pool: &DbManager,
    user_id: u64,
    reply_text: Option<String>,
    repeat_message: bool,
    roast_mode: bool,
) -> Result<(), sqlx::Error> {
    let user_id = user_id as i64;
    sqlx::query(
        "INSERT OR REPLACE INTO tracked_users (user_id, reply_text, repeat_message, roast_mode) VALUES (?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(reply_text)
    .bind(repeat_message)
    .bind(roast_mode)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn untrack_user(pool: &DbManager, user_id: u64) -> Result<(), sqlx::Error> {
    let user_id = user_id as i64;
    sqlx::query("DELETE FROM tracked_users WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_tracked_user(
    pool: &DbManager,
    user_id: u64,
) -> Result<Option<UserTracking>, sqlx::Error> {
    let user_id = user_id as i64;
    sqlx::query_as::<_, UserTracking>(
        "SELECT user_id, reply_text, repeat_message, roast_mode FROM tracked_users WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

pub async fn log_chat(pool: &DbManager, user_id: u64, content: &str) -> Result<(), sqlx::Error> {
    let user_id = user_id as i64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    sqlx::query("INSERT INTO chat_logs (user_id, content, timestamp) VALUES (?, ?, ?)")
        .bind(user_id)
        .bind(content)
        .bind(now)
        .execute(pool)
        .await?;
    Ok(())
}
