use sqlx::sqlite::SqlitePool;

use crate::{AppState, User};

pub async fn create_project(state: &AppState, user_id: i64, title: &str, slug: &str, desc: &str, thumb: &str) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO projects (user_uid, title, slug, description, thumbnail_path) VALUES (?, ?, ?, ?, ?)"
    )
    .bind(user_id)
    .bind(title)
    .bind(slug)
    .bind(desc)
    .bind(thumb)
    .execute(&state.db)
    .await?;

    // Return the ID of the newly created project
    Ok(result.last_insert_rowid())
}

pub async fn get_user(state: &AppState, user_id: i64) -> Option<User> {
    let result = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE uid = ?"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await;

    return result.unwrap_or(None);
}