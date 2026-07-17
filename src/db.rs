use crate::{AppState, Comment, LogEntry, Project, User, slug, week};

// TODO `SELECT name FROM sqlite_master WHERE type='table' AND name='users';`

pub async fn create_log(state: &AppState, project_id: i64, title: &str, number: i64) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO logs (project_uid, title, number, created_on) VALUES (?, ?, ?, ?)"
    )
        .bind(project_id)
        .bind(title)
        .bind(number)
        .bind(week::today())
        .execute(&state.db)
        .await?;
    return Ok(result.last_insert_rowid());
}

pub async fn create_project(state: &AppState, user_id: i64, title: &str, slug: &str, desc: &str) -> Result<i64, sqlx::Error> {
    assert!(slug::slug_valid(slug));
    let result = sqlx::query(
        "INSERT INTO projects (user_uid, title, slug, description, created_on) VALUES (?, ?, ?, ?, ?)"
    )
        .bind(user_id)
        .bind(title)
        .bind(slug)
        .bind(desc)
        .bind(week::today())
        .execute(&state.db)
        .await?;
    return Ok(result.last_insert_rowid());
}

pub async fn create_user(state: &AppState, username: &str, displayname: &str, password: &str, week_len: i64, logsday_weekday: i64) -> Result<i64, sqlx::Error> {
    assert!(slug::slug_valid(username));
    let result = sqlx::query(
        "INSERT INTO users (username, displayname, password, week_len, logsday_weekday, schedule_last_changed, created_on) VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
        .bind(username)
        .bind(displayname)
        .bind(password) // plaintext password go brrrr
        .bind(week_len)
        .bind(logsday_weekday)
        .bind(week::today())
        .bind(week::today())
        .execute(&state.db)
        .await?;
    return Ok(result.last_insert_rowid());
}

pub async fn create_comment_for_log(state: &AppState, log_uid: i64, user_uid: i64, text: &str) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO log_comments (log_uid, user_uid, text, created_on) VALUES (?, ?, ?, ?)",
    )
        .bind(log_uid)
        .bind(user_uid)
        .bind(text) // plaintext password go brrrr
        .bind(week::now())
        .execute(&state.db)
        .await?;
    return Ok(result.last_insert_rowid());
}

// Deleters

// return true on success
pub async fn delete_user(state: &AppState, user_uid: i64) -> bool {
    let result = sqlx::query("DELETE FROM users WHERE uid = ?;")
        .bind(user_uid)
        .execute(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
        return false;
    }
    return true;
}

// return true on success
pub async fn delete_project(state: &AppState, project_uid: i64) -> bool {
    let result = sqlx::query("DELETE FROM projects WHERE uid = ?;")
        .bind(project_uid)
        .execute(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
        return false;
    }
    return true;
}

// return true on success
pub async fn delete_log(state: &AppState, log_uid: i64) -> bool {
    let result = sqlx::query("DELETE FROM logs WHERE uid = ?;")
        .bind(log_uid)
        .execute(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
        return false;
    }
    return true;
}

// Updaters

pub async fn update_user_displayname(state: &AppState, user_uid: i64, new_displayname: &str) -> bool {
    let result = sqlx::query("UPDATE users SET displayname = ? WHERE uid = ?;")
        .bind(new_displayname)
        .bind(user_uid)
        .execute(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
        return false;
    }
    return true;
}

// Getters for `users` table

pub async fn get_user(state: &AppState, user_id: i64) -> Option<User> {
    let result = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE uid = ?"
    )
        .bind(user_id)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
    }
    return result.unwrap_or(None);
}

pub async fn get_user_by_username(state: &AppState, username: &str) -> Option<User> {
    let result = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?;")
        .bind(username)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
    }
    return result.unwrap_or(None);
}

pub async fn get_all_users(state: &AppState) -> Vec<User> {
    let users = sqlx::query_as::<_,User>("SELECT * FROM users;")
        .fetch_all(&state.db)
        .await;
    if let Err(e) = &users {
        println!("DB ERROR: {}", e);
    }
    return users.unwrap_or(vec![]);
}

// Getters for `projects` table

pub async fn get_project(state: &AppState, project_id: i64) -> Option<Project> {
    let project = sqlx::query_as::<_,Project>("SELECT * FROM projects WHERE uid = ?;")
        .bind(&project_id)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &project {
        println!("DB ERROR: {}", e);
    }
    return project.unwrap_or(None);
}

pub async fn get_user_projects(state: &AppState, user_id: i64) -> Vec<Project> {
    let projects = sqlx::query_as::<_,Project>("SELECT * FROM projects WHERE user_uid = ?;")
        .bind(&user_id)
        .fetch_all(&state.db)
        .await;
    if let Err(e) = &projects {
        println!("DB ERROR: {}", e);
    }
    return projects.unwrap_or(vec![]);
}

pub async fn get_project_by_slug(state: &AppState, user_id: i64, project_slug: &str) -> Option<Project> {
    let project = sqlx::query_as::<_,Project>("SELECT * FROM projects WHERE user_uid = ? AND slug = ?;")
        .bind(&user_id)
        .bind(&project_slug)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &project {
        println!("DB ERROR: {}", e);
    }
    return project.unwrap_or(None);
}

// Getters for `logs` table

pub async fn get_project_logs(state: &AppState, project_id: i64) -> Vec<LogEntry> {
    let logs = sqlx::query_as::<_,LogEntry>("SELECT * FROM logs WHERE project_uid = ? ORDER BY created_on DESC;")
        .bind(&project_id)
        .fetch_all(&state.db)
        .await;
    if let Err(e) = &logs {
        println!("DB ERROR: {}", e);
    }
    return logs.unwrap_or(vec![]);
}

pub async fn get_log_by_number(state: &AppState, project_id: i64, log_number: i64) -> Option<LogEntry> {
    let log = sqlx::query_as::<_,LogEntry>("SELECT * FROM logs WHERE project_uid = ? AND number = ?;")
        .bind(&project_id)
        .bind(log_number)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &log {
        println!("DB ERROR: {}", e);
    }
    return log.unwrap_or(None);
}

pub async fn get_log_uuid_pslug_lslug(state: &AppState, user_id: i64, project_slug: &str, log_number: i64) -> Option<LogEntry> {
    let p = get_project_by_slug(&state, user_id, project_slug).await;
    if let None = p { return None; }
    let p = p.unwrap();
    return get_log_by_number(&state, p.uid, log_number).await;
}

pub async fn get_last_log(state: &AppState, user_uid: i64) -> Option<LogEntry> {
    let log = sqlx::query_as::<_,LogEntry>("SELECT l.uid, l.project_uid, l.title, l.number, l.created_on
        FROM logs l JOIN projects p ON l.project_uid = p.uid WHERE p.user_uid = ? ORDER BY l.created_on DESC LIMIT 1;")
        .bind(user_uid)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &log {
        println!("DB ERROR: {}", e);
    }
    return log.unwrap_or(None);
}

pub async fn get_last_project_log_by_slug(state: &AppState, user_uid: i64, project_slug: &str) -> Option<LogEntry> {
    let log = sqlx::query_as::<_,LogEntry>("SELECT l.uid, l.project_uid, l.title, l.number, l.created_on
        FROM logs l JOIN projects p ON l.project_uid = p.uid WHERE p.user_uid = ? AND p.slug = ? ORDER BY l.created_on DESC, l.number DESC LIMIT 1;")
        .bind(user_uid)
        .bind(project_slug)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &log {
        println!("DB ERROR: {}", e);
    }
    return log.unwrap_or(None);
}

pub async fn _get_last_project_log(state: &AppState, project_uid: i64) -> Option<LogEntry> {
    let log = sqlx::query_as::<_,LogEntry>("SELECT * FROM logs WHERE project_uid = ? ORDER BY created_on DESC, number DESC LIMIT 1;")
        .bind(project_uid)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &log {
        println!("DB ERROR: {}", e);
    }
    return log.unwrap_or(None);
}

pub async fn update_log(state: &AppState, log_uid: i64, title: &str) -> Result<(), sqlx::Error> {
    let _ = sqlx::query("UPDATE logs SET title = ? WHERE uid = ?;")
        .bind(title)
        .bind(log_uid)
        .execute(&state.db)
        .await?;
    return Ok(());
}

pub async fn get_comments_for_log(state: &AppState, log_uid: i64,) -> Vec<Comment> {
    let comments = sqlx::query_as::<_, Comment>(
        r#"
        SELECT 
            u.displayname,
            u.username,
            c.text,
            c.created_on
        FROM log_comments c
        JOIN users u ON c.user_uid = u.uid
        WHERE c.log_uid = ?
        ORDER BY c.created_on DESC
        "#
    )
    .bind(log_uid)
    .fetch_all(&state.db)
    .await;
    if let Err(e) = &comments {
        println!("DB ERROR: {}", e);
    }
    return comments.unwrap_or(vec![]);
}

/* likes */

#[derive(Debug, Default, sqlx::FromRow)]
pub struct Likes {
    pub likes: i32,
    pub dislikes: i32,
}

pub async fn get_log_likes(state: &AppState, log_uid: i64) -> Likes {
    let likes = sqlx::query_as::<_,Likes>(
    "SELECT
            COUNT(CASE WHEN is_like = TRUE THEN 1 END) as likes,
            COUNT(CASE WHEN is_like = FALSE THEN 1 END) as dislikes
        FROM log_likes
        WHERE log_uid = ?;"
    )
        .bind(log_uid)
        .fetch_one(&state.db)
        .await;
    if let Err(e) = &likes {
        println!("DB ERROR: {}", e);
    }
    return likes.unwrap_or_default();
}

pub async fn get_project_likes(state: &AppState, project_uid: i64) -> Likes {
    let likes = sqlx::query_as::<_,Likes>(
    "SELECT
            COUNT(CASE WHEN is_like = TRUE THEN 1 END) as likes,
            COUNT(CASE WHEN is_like = FALSE THEN 1 END) as dislikes
        FROM project_likes
        WHERE project_uid = ?;"
    )
        .bind(project_uid)
        .fetch_one(&state.db)
        .await;
    if let Err(e) = &likes {
        println!("DB ERROR: {}", e);
    }
    return likes.unwrap_or_default();
}

pub async fn get_user_likes(state: &AppState, user_profile_uid: i64) -> Likes {
    let likes = sqlx::query_as::<_,Likes>(
    "SELECT
            COUNT(CASE WHEN is_like = TRUE THEN 1 END) as likes,
            COUNT(CASE WHEN is_like = FALSE THEN 1 END) as dislikes
        FROM user_likes
        WHERE user_profile_uid = ?;"
    )
        .bind(user_profile_uid)
        .fetch_one(&state.db)
        .await;
    if let Err(e) = &likes {
        println!("DB ERROR: {}", e);
    }
    return likes.unwrap_or_default();
}

#[derive(Debug, Default, sqlx::FromRow)]
pub struct Like {
    pub is_like: bool
}

// return Some(true)=like, return Some(false)=dislike, reutrn None=no reaction
pub async fn get_log_like(state: &AppState, user_uid: i64, log_uid: i64) -> Option<Like> {
    let like = sqlx::query_as::<_,Like>("SELECT is_like FROM log_likes WHERE user_uid = ? AND log_uid = ?;")
        .bind(user_uid)
        .bind(log_uid)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &like {
        println!("DB ERROR: {}", e);
    }
    return like.unwrap_or(None);
}

// Some(true)=like, return Some(false)=dislike, None=no reaction
pub async fn set_log_like(state: &AppState, user_uid: i64, log_uid: i64, like: Option<Like>) -> Result<(), sqlx::Error> {
    match like {
        Some(Like{is_like}) => {
            // like/dislike
            sqlx::query(
                r#"
                INSERT INTO log_likes (user_uid, log_uid, is_like)
                VALUES (?, ?, ?)
                ON CONFLICT(user_uid, log_uid) 
                DO UPDATE SET is_like = excluded.is_like
                "#)
                .bind(user_uid)
                .bind(log_uid)
                .bind(is_like)
                .execute(&state.db)
                .await?;
        }
        None => {
            // unlike
            sqlx::query("DELETE FROM log_likes WHERE user_uid = ? AND log_uid = ?")
                .bind(user_uid)
                .bind(log_uid)
                .execute(&state.db)
                .await?;
        }
    }
    Ok(())
}

// return Some(true)=like, return Some(false)=dislike, reutrn None=no reaction
pub async fn get_project_like(state: &AppState, user_uid: i64, project_uid: i64) -> Option<Like> {
    let like = sqlx::query_as::<_,Like>("SELECT is_like FROM project_likes WHERE user_uid = ? AND project_uid = ?;")
        .bind(user_uid)
        .bind(project_uid)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &like {
        println!("DB ERROR: {}", e);
    }
    return like.unwrap_or(None);
}

// Some(true)=like, return Some(false)=dislike, None=no reaction
pub async fn set_project_like(state: &AppState, user_uid: i64, project_uid: i64, like: Option<Like>) -> Result<(), sqlx::Error> {
    match like {
        Some(Like{is_like}) => {
            // like/dislike
            sqlx::query(
                r#"
                INSERT INTO project_likes (user_uid, project_uid, is_like)
                VALUES (?, ?, ?)
                ON CONFLICT(user_uid, project_uid) 
                DO UPDATE SET is_like = excluded.is_like
                "#)
                .bind(user_uid)
                .bind(project_uid)
                .bind(is_like)
                .execute(&state.db)
                .await?;
        }
        None => {
            // unlike
            sqlx::query("DELETE FROM project_likes WHERE user_uid = ? AND project_uid = ?")
                .bind(user_uid)
                .bind(project_uid)
                .execute(&state.db)
                .await?;
        }
    }
    Ok(())
}

// return Some(true)=like, return Some(false)=dislike, reutrn None=no reaction
pub async fn get_user_like(state: &AppState, user_uid: i64, user_profile_uid: i64) -> Option<Like> {
    let like = sqlx::query_as::<_,Like>("SELECT is_like FROM user_likes WHERE user_uid = ? AND user_profile_uid = ?;")
        .bind(user_uid)
        .bind(user_profile_uid)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &like {
        println!("DB ERROR: {}", e);
    }
    return like.unwrap_or(None);
}

// Some(true)=like, return Some(false)=dislike, None=no reaction
pub async fn set_user_like(state: &AppState, user_uid: i64, user_profile_uid: i64, like: Option<Like>) -> Result<(), sqlx::Error> {
    match like {
        Some(Like{is_like}) => {
            // like/dislike
            sqlx::query(
                r#"
                INSERT INTO user_likes (user_uid, user_profile_uid, is_like)
                VALUES (?, ?, ?)
                ON CONFLICT(user_uid, user_profile_uid) 
                DO UPDATE SET is_like = excluded.is_like
                "#)
                .bind(user_uid)
                .bind(user_profile_uid)
                .bind(is_like)
                .execute(&state.db)
                .await?;
        }
        None => {
            // unlike
            sqlx::query("DELETE FROM user_likes WHERE user_uid = ? AND user_profile_uid = ?")
                .bind(user_uid)
                .bind(user_profile_uid)
                .execute(&state.db)
                .await?;
        }
    }
    Ok(())
}