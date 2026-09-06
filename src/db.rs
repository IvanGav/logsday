use sqlx::Row;

use crate::{AppState, Comment, LogEntry, Project, User, slug, week};

// Query and create missing tables

const TABLES: std::sync::LazyLock<std::collections::HashMap<&str, &str>> = std::sync::LazyLock::new(||{[
("users",
"CREATE TABLE users (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    displayname TEXT NOT NULL,
    password TEXT NOT NULL,
    week_len INTEGER NOT NULL DEFAULT 8,
    logsday_weekday INTEGER NOT NULL DEFAULT 3, -- Logsday is between Wednesday and Thursday; Monday is 0; Sunday is 6/7
    schedule_last_changed INTEGER NOT NULL,
    email TEXT,
    admin BOOLEAN NOT NULL DEFAULT FALSE,
    created_on INTEGER NOT NULL
);"),
("projects",
"CREATE TABLE projects (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    user_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    description TEXT,
    created_on INTEGER NOT NULL,

    UNIQUE(user_uid, slug),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE
);"),
("logs",
"CREATE TABLE logs (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    project_uid INTEGER NOT NULL,
    title TEXT NOT NULL,
    number INTEGER NOT NULL, -- this log's sequential number in the project
    created_on INTEGER NOT NULL,

    UNIQUE(project_uid, number),
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);"),
("log_comments",
"CREATE TABLE log_comments (
    uid INTEGER PRIMARY KEY AUTOINCREMENT,
    log_uid INTEGER NOT NULL,
    user_uid INTEGER NOT NULL,
    text TEXT NOT NULL,
    created_on INTEGER NOT NULL,

    FOREIGN KEY (log_uid) REFERENCES logs(uid) ON DELETE CASCADE,
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE
);"),
("log_likes",
"CREATE TABLE log_likes (
    user_uid INTEGER NOT NULL,
    log_uid INTEGER NOT NULL,
    is_like BOOLEAN NOT NULL, -- like or dislike
    PRIMARY KEY (user_uid, log_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (log_uid) REFERENCES logs(uid) ON DELETE CASCADE
);"),
("project_likes",
"CREATE TABLE project_likes (
    user_uid INTEGER NOT NULL,
    project_uid INTEGER NOT NULL,
    is_like BOOLEAN NOT NULL, -- like or dislike
    PRIMARY KEY (user_uid, project_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (project_uid) REFERENCES projects(uid) ON DELETE CASCADE
);"),
("user_likes",
"CREATE TABLE user_likes (
    user_uid INTEGER NOT NULL,
    user_profile_uid INTEGER NOT NULL,
    is_like BOOLEAN NOT NULL, -- like or dislike
    PRIMARY KEY (user_uid, user_profile_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (user_profile_uid) REFERENCES users(uid) ON DELETE CASCADE
);"),
("user_follows",
"CREATE TABLE user_follows (
    user_uid INTEGER NOT NULL,
    user_profile_uid INTEGER NOT NULL,
    PRIMARY KEY (user_uid, user_profile_uid),
    FOREIGN KEY (user_uid) REFERENCES users(uid) ON DELETE CASCADE,
    FOREIGN KEY (user_profile_uid) REFERENCES users(uid) ON DELETE CASCADE
);")
].into()});

async fn verify_table_schema(state: &AppState, table_name: &str, expected_sql: &str) -> Option<bool> {
    let row = sqlx::query("SELECT sql FROM sqlite_master WHERE type='table' AND name=?")
        .bind(table_name)
        .fetch_optional(&state.db)
        .await.ok()??;
    let sql: String = row.get(0);
    let dialect = sqlparser::dialect::SQLiteDialect {};
    let expected_ast = sqlparser::parser::Parser::parse_sql(&dialect, expected_sql);
    if let Err(e) = expected_ast { println!("VERIFY TABLE ERROR: {}", e); return Some(false); }
    let expected_ast = expected_ast.unwrap();
    let actual_ast = sqlparser::parser::Parser::parse_sql(&dialect, &sql);
    if let Err(e) = actual_ast { println!("VERIFY TABLE ERROR: {}", e); return Some(false); }
    let actual_ast = actual_ast.unwrap();
    Some(expected_ast == actual_ast)
}

pub async fn create_and_verify_tables(state: &AppState) -> bool {
    let mut good = true;
    for table in TABLES.iter() {
        match verify_table_schema(state, table.0, table.1).await {
            Some(false) => {
                println!("Table {} exists, but has a different CREATE statement:\nRequired:\n{}", table.0, table.1);
                good = false;
            },
            None => {
                println!("Table {} does not exit. Creating.", table.0);
                if let Err(e) = sqlx::query(table.1).execute(&state.db).await {
                    println!("Could not create table {} - {}", table.0, e);
                    good = false;
                }
            },
            _ => {}
        }
    }
    return good;
}

// Creators

pub async fn create_log(state: &AppState, project_id: i64, title: &str, number: i64) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO logs (project_uid, title, number, created_on) VALUES (?, ?, ?, ?)"
    )
        .bind(project_id)
        .bind(title)
        .bind(number)
        .bind(week::now())
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
        .bind(week::now())
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
        .bind(password)
        .bind(week_len)
        .bind(logsday_weekday)
        .bind(week::now())
        .bind(week::now())
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
        .bind(text)
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

pub async fn delete_comment(state: &AppState, comment_uid: i64) -> bool {
    let result = sqlx::query("DELETE FROM log_comments WHERE uid = ?;")
        .bind(comment_uid)
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

pub async fn update_user_email(state: &AppState, user_uid: i64, new_email: &str) -> bool {
    let result = sqlx::query("UPDATE users SET email = ? WHERE uid = ?;")
        .bind(new_email)
        .bind(user_uid)
        .execute(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
        return false;
    }
    return true;
}

pub async fn update_log(state: &AppState, log_uid: i64, title: &str) -> Result<(), sqlx::Error> {
    let _ = sqlx::query("UPDATE logs SET title = ? WHERE uid = ?;")
        .bind(title)
        .bind(log_uid)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn update_project_title(state: &AppState, project_uid: i64, new_title: &str) -> Result<(), sqlx::Error> {
    let _ = sqlx::query("UPDATE projects SET title = ? WHERE uid = ?;")
        .bind(new_title)
        .bind(project_uid)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn update_project_description(state: &AppState, project_uid: i64, new_description: &str) -> Result<(), sqlx::Error> {
    let _ = sqlx::query("UPDATE projects SET description = ? WHERE uid = ?;")
        .bind(new_description)
        .bind(project_uid)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn update_comment(state: &AppState, comment_uid: i64, text: &str) -> Result<(), sqlx::Error> {
    let _ = sqlx::query("UPDATE log_comments SET text = ? WHERE uid = ?;")
        .bind(text)
        .bind(comment_uid)
        .execute(&state.db)
        .await?;
    Ok(())
}

// Getters for `users` table

pub async fn get_user(state: &AppState, user_id: i64) -> Option<User> {
    let result = sqlx::query_as::<_, User>(
        "SELECT uid, username, displayname, password, week_len, logsday_weekday, admin, created_on FROM users WHERE uid = ?"
    )
        .bind(user_id)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
    }
    return result.unwrap_or(None);
}

pub async fn get_user_email(state: &AppState, user_uid: i64) -> Option<String> {
    let result = sqlx::query_scalar::<_,String>(
        "SELECT email FROM users WHERE uid = ?"
    )
        .bind(user_uid)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
    }
    return result.unwrap_or(None);
}

pub async fn get_user_by_username(state: &AppState, username: &str) -> Option<User> {
    let result = sqlx::query_as::<_, User>(
        "SELECT uid, username, displayname, password, week_len, logsday_weekday, admin, created_on FROM users WHERE username = ?;"
    )
        .bind(username)
        .fetch_optional(&state.db)
        .await;
    if let Err(e) = &result {
        println!("DB ERROR: {}", e);
    }
    return result.unwrap_or(None);
}

pub async fn get_all_users(state: &AppState) -> Vec<User> {
    let users = sqlx::query_as::<_,User>(
        "SELECT uid, username, displayname, password, week_len, logsday_weekday, admin, created_on FROM users;"
    )
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

pub async fn get_comments_for_log(state: &AppState, log_uid: i64) -> Vec<Comment> {
    let comments = sqlx::query_as::<_, Comment>(
        r#"
        SELECT
            c.uid,
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

pub async fn get_comment_by_uid(state: &AppState, comment_uid: i64) -> Option<Comment> {
    let comment = sqlx::query_as::<_, Comment>(
        r#"
        SELECT
            c.uid,
            u.displayname,
            u.username,
            c.text,
            c.created_on
        FROM log_comments c
        JOIN users u ON c.user_uid = u.uid
        WHERE c.uid = ?
        ORDER BY c.created_on DESC
        "#
    )
    .bind(comment_uid)
    .fetch_one(&state.db)
    .await;
    if let Err(e) = &comment {
        println!("DB ERROR: {}", e);
    }
    return comment.ok();
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

/* other */

#[derive(Debug, Default, sqlx::FromRow)]
pub struct News {
    pub username: String,
    pub displayname: String,
    pub project_title: String,
    pub project_slug: String,
    pub log_title: String,
    pub log_number: i64,
    pub log_created_on: i64,
}

pub async fn get_news_for_user(state: &AppState, user_uid: i64) -> Vec<News> {
    let news = sqlx::query_as::<_,News>(r#"
        SELECT
            u.username,
            u.displayname,
            p.title AS project_title,
            p.slug AS project_slug,
            l.title AS log_title,
            l.number AS log_number,
            l.created_on AS log_created_on
        FROM user_follows uf
        JOIN projects p ON p.user_uid = uf.user_profile_uid
        JOIN users u ON u.uid = p.user_uid
        JOIN logs l ON l.project_uid = p.uid
        WHERE uf.user_uid = ?
        ORDER BY l.created_on DESC
        LIMIT 10
        "#)
        .bind(user_uid)
        .fetch_all(&state.db)
        .await.unwrap_or_default();
    news
}

pub async fn get_global_news(state: &AppState) -> Vec<News> {
    let news = sqlx::query_as::<_,News>(r#"
        SELECT
            u.username,
            u.displayname,
            p.title AS project_title,
            p.slug AS project_slug,
            l.title AS log_title,
            l.number AS log_number,
            l.created_on AS log_created_on
        FROM logs l
        JOIN projects p ON p.uid = l.project_uid
        JOIN users u ON u.uid = p.user_uid
        ORDER BY l.created_on DESC
        LIMIT 10
        "#)
        .fetch_all(&state.db)
        .await.unwrap_or_default();
    news
}

pub async fn is_following_user(state: &AppState, authd_user_uid: i64, profile_username: &str) -> Result<bool, sqlx::Error> {
    let profile_user = match get_user_by_username(&state, profile_username).await { Some(u) => u, None => return Err(sqlx::Error::RowNotFound) };
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM user_follows WHERE user_uid = ? AND user_profile_uid = ?)"
    )
        .bind(authd_user_uid)
        .bind(profile_user.uid)
        .fetch_one(&state.db)
        .await?;
    Ok(exists)
}

pub async fn follow_user(state: &AppState, authd_user: &User, profile_username: &str) -> Result<(), sqlx::Error> {
    let profile_user = match get_user_by_username(&state, profile_username).await { Some(u) => u, None => return Err(sqlx::Error::RowNotFound) };
    sqlx::query(
        r#"
        INSERT INTO user_follows (user_uid, user_profile_uid)
        VALUES (?, ?)
        "#)
        .bind(authd_user.uid)
        .bind(profile_user.uid)
        .execute(&state.db)
        .await?;
    Ok(())
}

pub async fn unfollow_user(state: &AppState, authd_user: &User, profile_username: &str) -> Result<(), sqlx::Error> {
    let profile_user = match get_user_by_username(&state, profile_username).await { Some(u) => u, None => return Err(sqlx::Error::RowNotFound) };
    sqlx::query("DELETE FROM user_follows WHERE user_uid = ? AND user_profile_uid = ?")
        .bind(authd_user.uid)
        .bind(profile_user.uid)
        .execute(&state.db)
        .await?;
    Ok(())
}

/*
SELECT
    u.username,
    u.displayname,
    p.title AS project_title,
    p.slug AS project_slug,
    l.title AS log_title,
    l.number AS log_number,
    l.created_on AS log_created_on
FROM logs l
JOIN projects p ON l.project_uid = p.uid
JOIN users u ON p.user_uid = u.uid
WHERE p.user_uid IN (
    -- Users you follow
    SELECT user_profile_uid 
    FROM user_follows 
    WHERE user_uid = ?1
) 
OR p.uid IN (
    -- Specific projects you follow
    SELECT project_uid 
    FROM project_follows 
    WHERE user_uid = ?1
)
ORDER BY l.created_on DESC
LIMIT 10;
*/

pub async fn get_all_user_emails_whose_logsday_is_today(state: &AppState) -> Vec<String> {
    let weekday_7_day_week = week::weekday_tz(7, 0);
    let weekday_8_day_week = week::weekday_tz(8, 0);
    let users = sqlx::query_scalar::<_,String>("SELECT email FROM users WHERE email != \"\" AND ((week_len = 7 AND logsday_weekday = ?) OR (week_len = 8 AND logsday_weekday = ?)) ;")
        .bind(weekday_7_day_week)
        .bind(weekday_8_day_week)
        .fetch_all(&state.db)
        .await;
    if let Err(e) = &users {
        println!("DB ERROR: {}", e);
    }
    return users.unwrap_or(vec![]);
}