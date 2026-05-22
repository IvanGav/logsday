use sqlx::sqlite::SqlitePool;

use crate::{AppState, LogEntry, Project, User, slug, week};

pub async fn create_log(state: &AppState, project_id: i64, title: &str, slug: &str, content_path: &str, thumb_path: &str) -> Result<i64, sqlx::Error> {
    assert!(slug::slug_valid(slug));
    let result = sqlx::query(
        "INSERT INTO logs (project_uid, title, slug, content_path, thumbnail_path, created_on) VALUES (?, ?, ?, ?, ?, ?)"
    )
        .bind(project_id)
        .bind(title)
        .bind(slug)
        .bind(content_path)
        .bind(thumb_path)
        .bind(week::today())
        .execute(&state.db)
        .await?;
    // Return the ID of the newly created log // TODO really?
    return Ok(result.last_insert_rowid());
}

pub async fn create_project(state: &AppState, user_id: i64, title: &str, slug: &str, desc: &str, thumb: &str) -> Result<i64, sqlx::Error> {
    assert!(slug::slug_valid(slug));
    let result = sqlx::query(
        "INSERT INTO projects (user_uid, title, slug, description, thumbnail_path, created_on) VALUES (?, ?, ?, ?, ?, ?)"
    )
        .bind(user_id)
        .bind(title)
        .bind(slug)
        .bind(desc)
        .bind(thumb)
        .bind(week::today())
        .execute(&state.db)
        .await?;
    // Return the ID of the newly created project // TODO really?
    return Ok(result.last_insert_rowid());
}

pub async fn create_user(state: &AppState, username: &str, displayname: &str, password: &str, week_len: i64, logsday_weekday: i64) -> Result<i64, sqlx::Error> {
    assert!(slug::slug_valid(username));
    let result = sqlx::query(
        "INSERT INTO users (username, displayname, password, week_len, logsday_weekday, schedule_last_changed) VALUES (?, ?, ?, ?, ?, ?)",
    )
        .bind(username)
        .bind(displayname)
        .bind(password) // plaintext password go brrrr
        .bind(week_len)
        .bind(logsday_weekday)
        .bind(week::today())
        .execute(&state.db)
        .await?;
    return Ok(result.last_insert_rowid()); // TODO really?
}

// Getters for `users` table

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

// Getters for `projects` table

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

// Getters for `logs` table

pub async fn get_project_logs(state: &AppState, project_id: i64) -> Vec<LogEntry> {
    let logs = sqlx::query_as::<_,LogEntry>("SELECT * FROM logs WHERE project_uid = ?;")
        .bind(&project_id)
        .fetch_all(&state.db)
        .await;
    return logs.unwrap_or(vec![]);
}

pub async fn get_log_by_slug(state: &AppState, project_id: i64, log_slug: &str) -> Option<LogEntry> {
    let log = sqlx::query_as::<_,LogEntry>("SELECT * FROM logs WHERE project_uid = ? AND slug = ?;")
        .bind(&project_id)
        .bind(&log_slug)
        .fetch_optional(&state.db)
        .await;
    return log.unwrap_or(None);
}

pub async fn get_log_uuid_pslug_lslug(state: &AppState, user_id: i64, project_slug: &str, log_slug: &str) -> Option<LogEntry> {
    let p = get_project_by_slug(&state, user_id, project_slug).await;
    if let None = p { return None; }
    let p = p.unwrap();
    return get_log_by_slug(&state, p.uid, log_slug).await;
}