use askama::Template;
use axum::{
    Form, Router, ServiceExt, body::Bytes, http::{HeaderMap, StatusCode, header}, response::{Html, IntoResponse, Redirect}, routing::{get, post}
};
use axum::extract::{Path, Query, State};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use std::{collections::HashMap, fs};
use tower_http::services::ServeDir;
use sqlx::sqlite::SqlitePool;
use serde::Deserialize;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::time};
use tower_sessions::Session;
use tower_http::normalize_path::NormalizePath;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

mod db;
mod slug;
mod week;

const ACCEPTED_THUMBNAIL_FILE_TYPES: [&str; 5] = ["image/png", "image/jpg", "image/jpeg", "image/gif", "image/webp"];
const WEEKDAY_NAMES: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Satruday", "Sunday"];

pub fn get_weekday_name(mut cur_day: i64) -> &'static str {
    if cur_day < 0 { cur_day += 7; }
    if cur_day > 6 { cur_day -= 7; }
    return WEEKDAY_NAMES[cur_day as usize];
}

pub fn render_markdown_to_html(markdown_input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);

    let parser = Parser::new_ext(markdown_input, options);

    let parser = parser.map(|event| match event {
        pulldown_cmark::Event::Html(text) | pulldown_cmark::Event::InlineHtml(text) => {
            pulldown_cmark::Event::Text(text)
        }
        other => other,
    });

    let mut in_custom_video = false;
    let mut video_url = "".to_string();

    let parser = parser.filter_map(|event| {
        match &event {
            // Catch the start of an image tag
            Event::Start(Tag::Image { dest_url, .. }) => {
                if dest_url.ends_with(".mp4") {
                    in_custom_video = true;
                    video_url = dest_url.to_string();
                    let mime_type = "video/mp4";
                    // Return the raw video player instead of the <img> tag
                    let video_html = format!(
                        r#"<video controls><source src="{}" type="{}"></video>"#, 
                        video_url, mime_type
                    );
                    Some(Event::Html(video_html.into()))
                } else {
                    Some(event)
                }
            }

            // Suppress the text inside the image brackets if it's a video
            Event::Text(_) => {
                if in_custom_video {
                    None 
                } else {
                    Some(event)
                }
            }

            // Catch the end of the image tag
            Event::End(TagEnd::Image { .. }) => {
                if in_custom_video {
                    in_custom_video = false; // Reset state machine flag
                    None // Drop the closing </img> token completely
                } else {
                    Some(event)
                }
            }

            // Pass everything else through normally
            _ => Some(event),
        }
    });

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    return html_output;
}

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
        .route("/project/{project_slug}/{log_number}", get(get_edit_log))
        .route("/new/project", get(get_new_project).post(post_new_project))
        .route("/new/log/{project_slug}", get(get_new_log).post(post_new_log))
        .route("/del/project/{project_slug}", post(post_del_project))
        .route("/del/log/{project_slug}/{log_number}", post(post_del_log))
        .route("/u/{username}", get(get_view_user))
        .route("/u/{username}/{project_slug}", get(get_view_project))
        .route("/u/{username}/{project_slug}/{log_number}", get(get_view_log))
        .route("/favicon.ico", get(get_favicon))
        .nest_service("/uploads", ServeDir::new("uploads/users"))
        .nest_service("/static", ServeDir::new("static"))
        .layer(session_layer)
        .with_state(state);

    let app = NormalizePath::trim_trailing_slash(app.into_service());
    let app = app.into_make_service();

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
    path: String,
    created_on: week::UnixTime,
}

#[derive(Debug, sqlx::FromRow, Default)]
struct LogEntry {
    uid: i64, // unique
    project_uid: i64,
    title: String,
    number: i64,
    path: String,
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

#[derive(Template)]
#[template(path = "message.html")]
struct MessageTemplate {
    message: String,
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
    let content_type: &str = content_type.as_ref().unwrap();

    if !ACCEPTED_THUMBNAIL_FILE_TYPES.contains(&content_type) { return "Unsupported file format".into_response(); }
    if !slug::slug_valid(&data.slug) { return "Project slug is invalid".into_response(); }
    let project_path = format!("uploads/users/{}/{}", &u.username, &data.slug);
    let thumbnail_path = format!("{}/{}", &project_path, "thumb.jpg");
    if let Ok(_) = db::create_project(&state, uid, &data.title, &data.slug, &data.description, &project_path).await {
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
struct EditProjectTemplate {
    username: String,
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

            let u = db::get_user(&state, uid).await;
            if let None = u { return hx_redirect("/login".into()).into_response(); }
            let username = u.unwrap().username;

            let logs = db::get_project_logs(&state, project.uid).await;
            let render = EditProjectTemplate{username, project, logs}.render();
            if let Ok(render) = render {
                return Html(render).into_response();
            }
            return generic_error().into_response();
        },
        None => Redirect::to("/login").into_response()
    }
}

// Route /project/{project_slug}/{log_number}
#[derive(Template)]
#[template(path = "editlog.html")]
struct EditLogTemplate {
    username: String,
    project_slug: String,
    log: LogEntry,
}

async fn get_edit_log(session: Session, State(state): State<AppState>, Path((project_slug, log_number)): Path<(String, String)>) -> impl IntoResponse {
    let user_id: Option<i64> = session.get("uid").await.unwrap();
    match user_id {
        Some(uid) => {
            let user = db::get_user(&state, uid).await;
            if let None = user { return "could not get user data".into_response(); }
            let user = user.unwrap();

            let log_number = log_number.parse::<i64>();
            if let Err(e) = log_number { return e.to_string().into_response(); }
            let log_number = log_number.unwrap();

            let log = db::get_log_uuid_pslug_lslug(&state, uid, &project_slug, log_number).await;
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

    let user = db::get_user(&state, uid).await.unwrap();

    if let Some(log) = db::get_last_log(&state, uid).await {
        if uid != 1 && week::days_since(log.created_on) < user.week_len { // user 1 is allowed to upload whenever; debug feature
            return Html("You've already uploaded a log this week! Go touch some logs and come back next week!").into_response();
        }
    }

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
    let content_type: &str = content_type.as_ref().unwrap();

    let log_number = db::get_last_project_log(&state, uid, &project_slug).await.unwrap_or_default().number + 1;

    if !ACCEPTED_THUMBNAIL_FILE_TYPES.contains(&content_type) { return "Unsupported file format".into_response(); }
    let log_path = format!("uploads/users/{}/{}/{}", &u.username, &project_slug, &log_number);
    let log_thumbnail_path = format!("{}/{}", &log_path, "thumb.jpg");
    let log_content_path = format!("{}/{}", &log_path, "index.md");
    let log_content_rendered_path = format!("{}/{}", &log_path, "index.html");
    match db::create_log(&state, project.uid, &data.title, log_number, &log_path).await {
        Ok(_) => {
            if let Ok(_) = tokio::fs::create_dir_all(log_path).await {
                if let Ok(_) = fs::write(log_thumbnail_path, &data.thumbnail.contents) {
                    let html_render = render_markdown_to_html(&data.content);
                    if let Ok(_) = fs::write(log_content_path, &data.content) {
                        if let Ok(_) = fs::write(log_content_rendered_path, &html_render) {
                            return hx_redirect(format!("/project/{}", project_slug)).into_response();
                        } else {
                            return "couldn't write rendered content".into_response();
                        }
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
            println!("{} ---- project/uid = '{}'/{}, log # = {}", e, project.title, project.uid, log_number);
            return e.to_string().into_response();
        }
    }
}

// Route /del/project/{project_slug}

async fn post_del_project(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>) -> impl IntoResponse {
    let uid: Option<i64> = session.get("uid").await.unwrap();
    if let None = uid { return hx_redirect("/login".into()).into_response(); }
    let uid = uid.unwrap();

    let u = db::get_user(&state, uid).await;
    if let None = u { return hx_redirect("/login".into()).into_response(); }
    let u = u.unwrap();

    let project = db::get_project_by_slug(&state, uid, &project_slug).await;
    if let None = project { return "Project not found".into_response(); }
    let project = project.unwrap();

    if db::delete_project(&state, project.uid).await {
        if let Err(e) = tokio::fs::remove_dir_all(format!("uploads/users/{}/{}", u.username, &project_slug)).await {
            return e.to_string().into_response();
        }
        return hx_redirect("/project".to_string()).into_response();
    }
    return "Project does not exist or cannot be deleted".into_response();
}

// Route /del/log/{project_slug}/{log_number}

async fn post_del_log(session: Session, State(state): State<AppState>, Path((project_slug, log_number)): Path<(String,String)>) -> impl IntoResponse {
    let uid: Option<i64> = session.get("uid").await.unwrap();
    if let None = uid { return hx_redirect("/login".into()).into_response(); }
    let uid = uid.unwrap();

    let u = db::get_user(&state, uid).await;
    if let None = u { return hx_redirect("/login".into()).into_response(); }
    let u = u.unwrap();

    let project = db::get_project_by_slug(&state, uid, &project_slug).await;
    if let None = project { return "Project not found".into_response(); }
    let project = project.unwrap();

    let log_number = log_number.parse::<i64>();
    if let Err(e) = log_number { return e.to_string().into_response(); }
    let log_number = log_number.unwrap();

    let log = db::get_log_by_slug(&state, project.uid, log_number).await;
    if let None = log { return "Log not found".into_response(); }
    let log = log.unwrap();

    if db::delete_log(&state, log.uid).await {
        if let Err(e) = tokio::fs::remove_dir_all(format!("uploads/users/{}/{}/{}", u.username, &project_slug, log_number)).await {
            return e.to_string().into_response();
        }
        return hx_redirect(format!("/project/{}", project_slug)).into_response();
    }
    return "Log does not exist or cannot be deleted".into_response();
}

// Route /u/{username}

#[derive(Template)]
#[template(path = "viewuser.html")]
struct ViewUserTemplate {
    owner: User,
    projects: Vec<Project>,
}

async fn get_view_user(State(state): State<AppState>, Path(username): Path<String>) -> impl IntoResponse {
    let u = db::get_user_by_username(&state, &username).await;
    if let None = u { return MessageTemplate{message: "User does not exist".to_string()}.render().unwrap().into_response(); }
    let u = u.unwrap();
    let projects = db::get_user_projects(&state, u.uid).await;
    return Html(ViewUserTemplate{owner: u, projects}.render().unwrap()).into_response();
}

// Route /u/{username}/{project_slug}

#[derive(Template)]
#[template(path = "viewproject.html")]
struct ViewProjectTemplate {
    owner: User,
    project: Project,
    logs: Vec<LogEntry>,
}

async fn get_view_project(State(state): State<AppState>, Path((username, project_slug)): Path<(String, String)>) -> impl IntoResponse {
    let u = db::get_user_by_username(&state, &username).await;
    if let None = u { return MessageTemplate{message: "User does not exist".to_string()}.render().unwrap().into_response(); }
    let u = u.unwrap();

    let project = db::get_project_by_slug(&state, u.uid, &project_slug).await;
    if let None = project { return "Project not found".into_response(); }
    let project = project.unwrap();

    let logs = db::get_project_logs(&state, project.uid).await;

    return Html(ViewProjectTemplate{owner: u, project, logs}.render().unwrap()).into_response();
}

// Route /u/{username}/{project_slug}/{log_number}

#[derive(Template)]
#[template(path = "viewlog.html")]
struct ViewLogTemplate {
    owner: User,
    project: Project,
    log: LogEntry,
}

async fn get_view_log(State(state): State<AppState>, Path((username, project_slug, log_number)): Path<(String, String, String)>) -> impl IntoResponse {
    let u = db::get_user_by_username(&state, &username).await;
    if let None = u { return MessageTemplate{message: "User does not exist".to_string()}.render().unwrap().into_response(); }
    let u = u.unwrap();

    let project = db::get_project_by_slug(&state, u.uid, &project_slug).await;
    if let None = project { return "Project not found".into_response(); }
    let project = project.unwrap();

    let log_number = log_number.parse::<i64>();
    if let Err(e) = log_number { return e.to_string().into_response(); }
    let log_number = log_number.unwrap();

    let log = db::get_log_by_slug(&state, project.uid, log_number).await;
    if let None = log { return "Log not found".into_response(); }
    let log = log.unwrap();

    return Html(ViewLogTemplate{owner: u, project, log}.render().unwrap()).into_response();
}

// Route /new/log/{project_slug}/upload

#[derive(TryFromMultipart)]
struct LogMediaUploadRequest {
    #[form_data(field_name = "file", limit = "50MB")]
    file: FieldData<Bytes>,
}

async fn post_new_log_media_upload(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>, data: TypedMultipart<LogMediaUploadRequest>) -> impl IntoResponse {
    let uid: Option<i64> = session.get("uid").await.unwrap();
    if let None = uid { return hx_redirect("/login".into()).into_response(); }
    let uid = uid.unwrap();

    let u = db::get_user(&state, uid).await;
    if let None = u { return hx_redirect("/login".into()).into_response(); }
    let u = u.unwrap();

    let project = db::get_project_by_slug(&state, uid, &project_slug).await;
    if let None = project { return "Project not found".into_response(); }
    let project = project.unwrap();

    let content_type = &data.file.metadata.content_type;
    if let None = content_type { return "Could not get file type".into_response(); }
    let content_type: &str = content_type.as_ref().unwrap();

    let log_number = db::get_last_project_log(&state, uid, &project.slug).await.unwrap_or_default().number + 1;
    let file_name = if let Some(name) = &data.file.metadata.file_name { name } else { "file" };

    if !ACCEPTED_THUMBNAIL_FILE_TYPES.contains(&content_type) { return "Unsupported file format".into_response(); }
    let log_path = format!("uploads/users/{}/{}/{}", &u.username, &project_slug, &log_number);
    if let Ok(_) = tokio::fs::create_dir_all(&log_path).await {
        if let Ok(_) = fs::write(format!("{}/{}", &log_path, &file_name), &data.file.contents) {
            return "Ok".into_response();
        } else {
            return "couldn't write file".into_response();
        }
    } else {
        return "couldn't create log dir".into_response();
    }
}
