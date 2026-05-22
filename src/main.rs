use askama::Template;
use axum::{
    Form, Router, body::Bytes, http::{HeaderMap, StatusCode, header}, response::{Html, IntoResponse, Redirect}, routing::get
};
use axum::extract::{Path, Query, State};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use std::{collections::HashMap, fs};
use tower_http::services::ServeDir;
use sqlx::sqlite::SqlitePool;
use serde::Deserialize;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::time};
use tower_sessions::Session;

mod db;
mod slug;
mod week;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[tokio::main]
async fn main() {
    let db_pool = SqlitePool::connect("sqlite:sqlite.db")
        .await
        .expect("Could not connect to database");

    let session_store = MemoryStore::default(); // store user sessions to memory for now
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // set to true later when have HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(1)));

    let state = AppState { db: db_pool };

    let app = Router::new()
        .route("/", get(landing))
        .route("/signup", get(get_signup).post(post_signup))
        .route("/login", get(get_login).post(post_login))
        .route("/project", get(get_project_list))
        .route("/project/{project_slug}", get(get_edit_project))
        .route("/project/{project_slug}/{log_slug}", get(get_edit_log))
        .route("/new/project", get(get_new_project).post(post_new_project))
        .route("/new/log/{project_slug}", get(get_new_log).post(post_new_log))
        .route("/favicon.ico", get(get_favicon))
        .nest_service("/uploads", ServeDir::new("uploads/users"))
        .nest_service("/static", ServeDir::new("static"))
        .layer(session_layer)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn generic_error() -> impl IntoResponse {
    return Html("Oops, something went wrong... Go touch some logs in the meantime.").into_response();
}

fn hx_redirect(route: String) -> impl IntoResponse {
    return ([("HX-Redirect", route)], "Redirecting...").into_response();
}

#[derive(Debug, sqlx::FromRow)]
struct User {
    uid: i64, // unique
    username: String, // unique
    displayname: String,
    password: String,
    week_len: i64,
    logsday_weekday: i64,
    schedule_last_changed: week::UnixTime,
}

#[derive(Debug, sqlx::FromRow)]
struct Project {
    uid: i64, // unique
    user_uid: i64,
    title: String,
    slug: String,
    description: String, // nullable
    thumbnail_path: String,
    created_on: week::UnixTime,
}

// for now, let's exclude updates from existing, i don't want to worry about 2 types of logs for now
#[derive(Debug, sqlx::FromRow)]
struct LogEntry {
    uid: i64, // unique
    project_uid: i64,
    title: String,
    slug: String,
    content_path: String,
    thumbnail_path: String,
    created_on: week::UnixTime,
}

async fn testing(Query(params): Query<HashMap<String, String>>, Path(user_id): Path<u32>) -> String {
    let mut a: String = user_id.to_string();
    a.push('\t');
    for s in params {
        a.push_str(&s.0);
        a.push(':');
        a.push_str(&s.1);
        a.push(',');
    }
    return a;
}

// Route /

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate;

async fn landing() -> impl IntoResponse {
    let render = LandingTemplate.render();
    if let Ok(render) = render {
        return Html(render).into_response();
    }
    return generic_error().into_response();
}

// Route /favicon.ico

async fn get_favicon() -> impl IntoResponse {
    // Bake the file into the binary at compile time - pretty cool. Did you know I like Rust?
    let bytes = include_bytes!("../static/favicon.ico");
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/x-icon".parse().unwrap());
    (headers, bytes)
}

// Route /new/project

#[derive(Template)]
#[template(path = "newproject.html")]
struct NewProjectTemplate;

async fn get_new_project(session: Session) -> impl IntoResponse {
    let uid: Option<i64> = session.get("uid").await.unwrap();
    if let None = uid { return Redirect::to("/login").into_response(); }
    let render = NewProjectTemplate.render();
    if let Ok(render) = render {
        return Html(render).into_response();
    }
    return generic_error().into_response();
}

#[derive(TryFromMultipart)]
struct NewProjectRequest {
    #[form_data(field_name = "title")]
    title: String,
    #[form_data(field_name = "slug")]
    slug: String,
    #[form_data(field_name = "description")]
    description: String,
    #[form_data(field_name = "thumbnail", limit = "2MB")]
    thumbnail: FieldData<Bytes>,
}

async fn post_new_project(State(state): State<AppState>, session: Session, data: TypedMultipart<NewProjectRequest>) -> impl IntoResponse {
    let uid: Option<i64> = session.get("uid").await.unwrap();
    if let None = uid { return hx_redirect("/login".into()).into_response(); }
    let uid = uid.unwrap();
    let u = db::get_user(&state, uid).await;
    if let None = u { return hx_redirect("/login".into()).into_response(); }
    let u = u.unwrap();
    let content_type = &data.thumbnail.metadata.content_type;
    if let None = content_type { return "Could not get file type".into_response(); }
    let content_type = content_type.as_ref().unwrap();
    if content_type != "image/jpg" && content_type != "image/jpeg" && content_type != "image/png" { return "Unsupported file format".into_response(); }
    if !slug::slug_valid(&data.slug) { return "Project slug is invalid".into_response(); }
    let project_path = format!("uploads/users/{}/{}", &u.username, &data.slug);
    let thumbnail_path = format!("{}/{}", &project_path, "thumb.jpg");
    if let Ok(_) = db::create_project(&state, uid, &data.title, &data.slug, &data.description, &thumbnail_path).await {
        if let Ok(_) = tokio::fs::create_dir_all(project_path).await {
            if let Ok(_) = fs::write(thumbnail_path, &data.thumbnail.contents) {
                return hx_redirect("/project".into()).into_response();
            }
        }
    }
    return generic_error().into_response();
}

// Route /signup
#[derive(Template)]
#[template(path = "signup.html")]
struct SignupTemplate;

async fn get_signup() -> impl IntoResponse {
    let render = SignupTemplate.render();
    if let Ok(render) = render {
        return Html(render).into_response();
    }
    return generic_error().into_response();
}

#[derive(Deserialize)]
struct SignupSubmission {
    username: String,
    displayname: String,
    password: String,
}

async fn post_signup(
    session: Session,
    State(state): State<AppState>,
    Form(form): Form<SignupSubmission>,
) -> impl IntoResponse {
    if !slug::slug_valid(&form.username) {
        return generic_error().into_response();
    }
    let result = db::create_user(&state, &form.username, &form.displayname, &form.password, 8, 3).await;
    match result {
        Ok(_) => {
            if let Ok(_) = tokio::fs::create_dir_all(format!("uploads/users/{}", form.username)).await {
                let u = db::get_user_by_username(&state, &form.username).await;
                if let None = u { return (StatusCode::INTERNAL_SERVER_ERROR, "Couldn't find user after creating").into_response(); }
                let uid = u.unwrap().uid;
                session.insert("uid", uid).await.unwrap();
                return hx_redirect("/project".into()).into_response();
            } else {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Could not create user directory").into_response();
            }
        }
        Err(e) => {
            if let Some(db_err) = e.as_database_error() {
                if db_err.is_unique_violation() {
                    return (StatusCode::BAD_REQUEST, "Username already taken").into_response();
                }
            }
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database fail").into_response();
        }
    }
}

// Route /login
#[derive(Template)]
#[template(path = "login.html")]
struct LoginTemplate;

async fn get_login() -> impl IntoResponse {
    let render = LoginTemplate.render();
    if let Ok(render) = render {
        return Html(render).into_response();
    }
    return generic_error().into_response();
}

#[derive(Deserialize)]
struct LoginSubmission {
    username: String,
    password: String,
}

async fn post_login(
    session: Session,
    State(state): State<AppState>,
    Form(form): Form<LoginSubmission>,
) -> impl IntoResponse {
    let user = db::get_user_by_username(&state, &form.username).await;
    if let Some(u) = user {
        if u.password == form.password {
            session.insert("uid", u.uid).await.unwrap();
            return hx_redirect("/project".into()).into_response();
        }
    }
    return "Incorrect Username or Password".into_response();
}

// Route /project
#[derive(Template)]
#[template(path = "projectlist.html")]
struct ProjectListTemplate {
    projects: Vec<Project>
}

async fn get_project_list(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    let user_id: Option<i64> = session.get("uid").await.unwrap();
    if let None = user_id {
        return Redirect::to("/login").into_response();
    }
    let user_id = user_id.unwrap();
    let projects = db::get_user_projects(&state, user_id).await;
    let render = ProjectListTemplate{projects}.render();
    if let Ok(render) = render {
        return Html(render).into_response();
    }
    return generic_error().into_response();
}

// Route /project/{project_slug}
#[derive(Template)]
#[template(path = "editproject.html")]
struct MyProjectTemplate {
    project: Project,
    logs: Vec<LogEntry>,
}

async fn get_edit_project(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>) -> impl IntoResponse {
    let user_id: Option<i64> = session.get("uid").await.unwrap();
    match user_id {
        Some(uid) => {
            let project = db::get_project_by_slug(&state, uid, &project_slug).await;
            if let None = project { return "Project not found".into_response(); }
            let project = project.unwrap();
            let logs = db::get_project_logs(&state, project.uid).await;
            let render = MyProjectTemplate{project, logs}.render();
            if let Ok(render) = render {
                return Html(render).into_response();
            }
            return generic_error().into_response();
        },
        None => Redirect::to("/login").into_response()
    }
}

// Route /project/{project_slug}/{log_slug}
#[derive(Template)]
#[template(path = "editlog.html")]
struct EditLogTemplate {
    username: String,
    project_slug: String,
    log: LogEntry,
}

async fn get_edit_log(session: Session, State(state): State<AppState>, Path((project_slug, log_slug)): Path<(String, String)>) -> impl IntoResponse {
    let user_id: Option<i64> = session.get("uid").await.unwrap();
    match user_id {
        Some(uid) => {
            let user = db::get_user(&state, uid).await;
            if let None = user { return "could not get user data".into_response(); }
            let user = user.unwrap();

            let log = db::get_log_uuid_pslug_lslug(&state, uid, &project_slug, &log_slug).await;
            if let None = log { return "Log not found".into_response(); }
            let log = log.unwrap();

            let render = EditLogTemplate{username: user.username, project_slug, log}.render();
            if let Ok(render) = render {
                return Html(render).into_response();
            }
            return generic_error().into_response();
        },
        None => Redirect::to("/login").into_response()
    }
}

// Route /new/log/{project_slug}

#[derive(Template)]
#[template(path = "newlog.html")]
struct NewLogTemplate {
    project: Project
}

async fn get_new_log(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>) -> impl IntoResponse {
    let uid = session.get("uid").await.unwrap();
    if let None = uid { return Redirect::to("/login").into_response(); }
    let uid = uid.unwrap();
    let project = db::get_project_by_slug(&state, uid, &project_slug).await;
    if let None = project { return Html("Project does not exist").into_response(); }
    let project = project.unwrap();
    let render = NewLogTemplate{project}.render();
    if let Ok(render) = render {
        return Html(render).into_response();
    }
    return generic_error().into_response();
}

#[derive(TryFromMultipart)]
struct NewLogRequest {
    #[form_data(field_name = "title")]
    title: String,
    #[form_data(field_name = "slug")]
    slug: String,
    #[form_data(field_name = "content")]
    content: String,
    #[form_data(field_name = "thumbnail", limit = "2MB")]
    thumbnail: FieldData<Bytes>,
}

async fn post_new_log(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>, data: TypedMultipart<NewLogRequest>) -> impl IntoResponse {
    let uid: Option<i64> = session.get("uid").await.unwrap();
    if let None = uid { return hx_redirect("/login".into()).into_response(); }
    let uid = uid.unwrap();

    let u = db::get_user(&state, uid).await;
    if let None = u { return hx_redirect("/login".into()).into_response(); }
    let u = u.unwrap();

    let project = db::get_project_by_slug(&state, uid, &project_slug).await;
    if let None = project { return "Project not found".into_response(); }
    let project = project.unwrap();

    let content_type = &data.thumbnail.metadata.content_type;
    if let None = content_type { return "Could not get file type".into_response(); }
    let content_type = content_type.as_ref().unwrap();

    if content_type != "image/jpg" && content_type != "image/jpeg" && content_type != "image/png" { return "Unsupported file format".into_response(); }
    if !slug::slug_valid(&data.slug) { return "Project slug is invalid".into_response(); }
    let log_path = format!("uploads/users/{}/{}/{}", &u.username, &project_slug, &data.slug);
    let log_thumbnail_path = format!("{}/{}", &log_path, "thumb.jpg");
    let log_content_path = format!("{}/{}", &log_path, "content.md");
    match db::create_log(&state, project.uid, &data.title, &data.slug, &log_content_path, &log_thumbnail_path).await {
        Ok(_) => {
            if let Ok(_) = tokio::fs::create_dir_all(log_path).await {
                if let Ok(_) = fs::write(log_thumbnail_path, &data.thumbnail.contents) {
                    if let Ok(_) = fs::write(log_content_path, &data.content) {
                        return hx_redirect(format!("/project/{}", project_slug)).into_response();
                    } else {
                        return "couldn't write content".into_response();
                    }
                } else {
                    return "couldn't write thumbnail".into_response();
                }
            } else {
                return "couldn't create log dir".into_response();
            }
        },
        Err(e) => {
            return e.as_database_error().unwrap().to_string().into_response();
        }
    }
}

/*
SELECT l.created_at 
FROM logs l
JOIN projects p ON l.project_uid = p.uid
WHERE p.user_uid = ? 
ORDER BY l.created_at DESC 
LIMIT 1;
*/