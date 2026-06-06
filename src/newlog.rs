use axum::Json;
use serde_json::json;

use crate::db;
// use crate::filestuff;
use crate::week;
use crate::{User, Project, LogEntry, AppState};

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
    return Json(json!({
        "filename": file_name,
        "filesize": incoming_size,
        "filepath": log_file_web_path
    }));
}