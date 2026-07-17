use axum::Json;
use serde_json::json;

use crate::db;
use crate::filestuff::get_extension;
use crate::week;
use crate::{User, Project, AppState};

pub enum NewlogResult {
    New(Project, i64),
    NotLogsday,
    ProjectNotFound,
    AlreadyUploaded,
}

pub async fn newlog_num(state: &AppState, user: &User, project_slug: &str) -> NewlogResult {
    let proj = match db::get_project_by_slug(state, user.uid, project_slug).await { Some(proj) => proj, None => { return NewlogResult::ProjectNotFound; } };
    let last_project_log = db::get_last_project_log_by_slug(&state, user.uid, &project_slug).await.unwrap_or_default();
    if !user.admin {
        if !week::is_logsday(user.week_len, user.logsday_weekday) {
            return NewlogResult::NotLogsday;
        }
        let last_log = db::get_last_log(&state, user.uid).await.unwrap_or_default();
        if week::days_since(last_log.created_on) == 0 {
            return NewlogResult::AlreadyUploaded;
        }
    }
    return NewlogResult::New(proj, last_project_log.number + 1);
}

pub fn file_response(file_name: &str, incoming_size: u64, log_file_web_path: &str) -> Json<serde_json::Value> {
    let log_file_web_path = askama::filters::urlencode(log_file_web_path).unwrap().to_string();
    return Json(json!({
        "filename": file_name,
        "filesize": incoming_size,
        "filepath": log_file_web_path
    }));
}

pub fn error_json(message: &str) -> Json<serde_json::Value> {
    Json::from(json!({
        "error": message,
    }))
}

pub fn get_existing_files(user: &User, project: &Project, log_num: i64) -> serde_json::Value {
    let dir = format!("uploads/users/{}/{}/{}", user.username, project.slug, log_num);
    match std::fs::read_dir(&dir) {
        Ok(mut dir) => {
            let mut list = vec![];
            while let Some(file) = dir.next() {
                let metadata = file.as_ref().unwrap().metadata().unwrap();
                let filename = file.unwrap().file_name().to_string_lossy().into_owned();
                if metadata.is_file() && filename != "index.html" && filename != "index.md" && get_extension(&filename) != Some("tmp") {
                    list.push(file_response(&filename, metadata.len(), &format!("/uploads/{}/{}/{}/{}", user.username, project.slug, log_num, &filename)).0);
                }
            }
            return serde_json::Value::Array(list);
        },
        Err(_) => { return serde_json::Value::Array(vec![]); }
    }
}