use axum::Json;
use serde_json::json;

use crate::db;
// use crate::filestuff;
use crate::week;
use crate::{User, Project, LogEntry, AppState};

pub enum NewlogResult {
    New(Project, i64),
    Edit(Project, LogEntry),
    NotLogsday,
    ProjectNotFound,
    AlreadyUploadedForProject { project_uid: i64 },
}

pub async fn newlog_num(state: &AppState, user: &User, project_slug: &str) -> NewlogResult {
    let proj = match db::get_project_by_slug(state, user.uid, project_slug).await { Some(proj) => proj, None => { return NewlogResult::ProjectNotFound; } };
    let last_project_log = db::get_last_project_log(&state, user.uid, &project_slug).await.unwrap_or_default();
    if !user.admin {
        if !week::is_logsday(user.week_len, user.logsday_weekday) {
            return NewlogResult::NotLogsday;
        }
        if let Some(last_log) = db::get_last_log(&state, user.uid).await {
            if last_log.project_uid != last_project_log.project_uid && week::days_since(last_log.created_on) == 0 {
                return NewlogResult::AlreadyUploadedForProject { project_uid: last_log.project_uid };
            }
        }
    }
    if week::days_since(last_project_log.created_on) == 0 {
        return NewlogResult::Edit(proj, last_project_log);
    }
    return NewlogResult::New(proj, last_project_log.number + 1);
}

pub fn log_response(file_name: &str, incoming_size: u64, log_file_web_path: &str) -> Json<serde_json::Value> {
    return Json(json!({
        "filename": file_name,
        "filesize": incoming_size,
        "filepath": log_file_web_path
    }));
}