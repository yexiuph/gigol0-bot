#[derive(sqlx::FromRow)]
pub struct UserProfile {
    pub user_id: i64,
    pub points: i64,
    pub last_daily: Option<i64>,
}
