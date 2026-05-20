use sqlx::sqlite::SqlitePool;

use crate::{AppState, Project, User, slug};

pub async fn create_project(state: &AppState, user_id: i64, title: &str, slug: &str, desc: &str, thumb: &str) -> Result<i64, sqlx::Error> {
    assert!(slug::slug_valid(slug));
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
    return Ok(result.last_insert_rowid());
}

pub async fn create_user(state: &AppState, username: &str, displayname: &str, password: &str) -> Result<i64, sqlx::Error> {
    assert!(slug::slug_valid(username));
    let result = sqlx::query(
        "INSERT INTO users (username, displayname, password) VALUES (?, ?, ?)",
    )
        .bind(username)
        .bind(displayname)
        .bind(password) // plaintext password go brrrr
        .execute(&state.db)
        .await?;
    return Ok(result.last_insert_rowid());
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

pub async fn get_user_by_username(state: &AppState, username: &str) -> Option<User> {
    let result = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?;")
        .bind(username)
        .fetch_optional(&state.db)
        .await;
    return result.unwrap_or(None);
}

pub async fn get_project(state: &AppState, project_id: i64) -> Option<Project> {
    let projects = sqlx::query_as::<_,Project>("SELECT * FROM projects WHERE uid = ?;")
        .bind(&project_id)
        .fetch_optional(&state.db)
        .await;
    return projects.unwrap_or(None);
}

pub async fn get_user_projects(state: &AppState, user_id: i64) -> Vec<Project> {
    let projects = sqlx::query_as::<_,Project>("SELECT * FROM projects WHERE user_uid = ?;")
        .bind(&user_id)
        .fetch_all(&state.db)
        .await;
    return projects.unwrap_or(vec![]);
}

pub async fn get_project_by_slug(state: &AppState, user_id: i64, project_slug: &str) -> Option<Project> {
    let projects = sqlx::query_as::<_,Project>("SELECT * FROM projects WHERE user_uid = ? AND slug = ?;")
        .bind(&user_id)
        .bind(&project_slug)
        .fetch_optional(&state.db)
        .await;
    return projects.unwrap_or(None);
}