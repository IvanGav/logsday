use askama::Template;
use axum::{
    Form, Json, Router, ServiceExt, body::Bytes, extract::DefaultBodyLimit, http::{HeaderMap, StatusCode, header}, response::{Html, IntoResponse, Redirect}, routing::{delete, get, post}
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

use crate::{filestuff::MediaType, newlog::NewlogResult};

mod db;
mod slug;
mod week;
mod filestuff;
mod newlog;

const ACCEPTED_THUMBNAIL_FILE_TYPES: [&str; 5] = ["image/png", "image/jpg", "image/jpeg", "image/gif", "image/webp"];
const WEEKDAY_NAMES: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Satruday", "Sunday"];

pub fn get_weekday_name(mut cur_day: i64) -> &'static str {
    if cur_day < 0 { cur_day += 7; }
    if cur_day > 6 { cur_day -= 7; }
    return WEEKDAY_NAMES[cur_day as usize];
}

pub fn msg_html(message: String) -> Html<String> {
    Html(MessageTemplate { message }.render().unwrap())
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
        .route("/debug", get(get_debug))
        .route("/", get(landing))
        .route("/signup", get(get_signup).post(post_signup))
        .route("/login", get(get_login).post(post_login))
        .route("/logout", get(get_logout))
        .route("/project", get(get_project_list))
        .route("/project/{project_slug}", get(get_edit_project))
        .route("/project/{project_slug}/{log_number}", get(get_edit_log))
        .route("/new/project", get(get_new_project).post(post_new_project))
        .route("/new/log/{project_slug}", get(get_new_log).post(post_new_log))
        .route("/new/log/{project_slug}/upload", post(post_new_log_media_upload))
        .route("/new/log/{project_slug}/delete/{delete_filename}", delete(delete_log_media_delete))
        .route("/del/project/{project_slug}", post(post_del_project))
        .route("/del/log/{project_slug}/{log_number}", post(post_del_log))
        .route("/u/{username}", get(get_view_user))
        .route("/u/{username}/{project_slug}", get(get_view_project))
        .route("/u/{username}/{project_slug}/{log_number}", get(get_view_log))
        .route("/bits/nav-user", get(get_nav_user_bit))
        .route("/favicon.ico", get(get_favicon))
        .nest_service("/uploads", ServeDir::new("uploads/users"))
        .nest_service("/static", ServeDir::new("static"))
        .layer(session_layer)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024)) // do not allow uploads of over 100MB; should also be enforced on client side
        .with_state(state);

    let app = NormalizePath::trim_trailing_slash(app.into_service());
    let app = app.into_make_service();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3009").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn generic_error() -> impl IntoResponse {
    return Html("Oops, something went wrong... Go touch some logs in the meantime.").into_response();
}

fn hx_redirect(route: &str) -> impl IntoResponse {
    return ([("HX-Redirect", route)], "Redirecting...").into_response();
}

fn redirect_login() -> impl IntoResponse {
    return Redirect::to("/login").into_response();
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
    admin: bool,
    created_on: week::UnixTime,
}

#[derive(Debug, sqlx::FromRow)]
struct Project {
    uid: i64, // unique
    user_uid: i64,
    title: String,
    slug: String,
    description: String,
    created_on: week::UnixTime,
}

#[derive(Debug, sqlx::FromRow, Default)]
struct LogEntry {
    uid: i64, // unique
    project_uid: i64,
    title: String,
    number: i64,
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

// Route /debug

#[derive(Template)]
#[template(path = "debug.html")]
struct DebugTemplate;
async fn get_debug() -> impl IntoResponse { Html(DebugTemplate.render().unwrap()).into_response() }

// Route /

#[derive(Template)]
#[template(path = "landing.html")]
struct LandingTemplate {
    user: Option<User>,
    display_users: Vec<User>,
}

async fn landing(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    let user = if let Some(uid) = session.get::<i64>("uid").await.unwrap() { db::get_user(&state, uid).await } else { None };
    let display_users = db::get_all_users(&state).await;

    let render = LandingTemplate { user, display_users }.render();
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
    #[form_data(field_name = "thumbnail", limit = "10MB")]
    thumbnail: FieldData<Bytes>,
}

async fn post_new_project(State(state): State<AppState>, session: Session, data: TypedMultipart<NewProjectRequest>) -> impl IntoResponse {
    let pslug: &str = if data.slug.len() == 0 { &slug::slug_from(&data.title) } else { &data.slug };

    if data.title.len() > 255 || pslug.len() > 255 { return "title or slug too long".into_response(); }
    if data.description.len() > 65535 { return "description too long".into_response(); }

    let uid = session.get::<i64>("uid").await.unwrap();
    if let None = uid { return "You're not logged in".into_response(); }
    let uid = uid.unwrap();

    let u = db::get_user(&state, uid).await;
    if let None = u { return "You're not logged in".into_response(); }
    let u = u.unwrap();

    let content_type = &data.thumbnail.metadata.content_type;
    if let None = content_type { return "Could not get file type".into_response(); }
    let content_type: &str = content_type.as_ref().unwrap();

    if filestuff::mime_media_type(content_type) != MediaType::Image { return "Unsupported thumbnail file format".into_response(); }
    if !slug::slug_valid(&pslug) { return "Project slug is invalid".into_response(); }
    let project_path = format!("uploads/users/{}/{}", &u.username, &pslug);
    let thumbnail_path = format!("{}/{}", &project_path, "thumb.webp");

    let webp_img = filestuff::convert_to_webp(&data.thumbnail.contents);
    if let Err(e) = webp_img { return e.to_string().into_response(); }
    let webp_img = webp_img.unwrap();

    if let Ok(_) = db::create_project(&state, uid, &data.title, &pslug, &data.description).await {
        if let Ok(_) = tokio::fs::create_dir_all(project_path).await {
            if let Ok(_) = fs::write(thumbnail_path, &webp_img) {
                return hx_redirect("/project").into_response();
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

#[derive(Deserialize, Debug)]
struct SignupSubmission {
    displayname: String,
    username: String,
    password: String,
    week_len: i64,
    logsday_weekday: i64,
}

async fn post_signup(
    session: Session,
    State(state): State<AppState>,
    Form(form): Form<SignupSubmission>,
) -> impl IntoResponse {
    let displayname = form.displayname.trim();
    if displayname.is_empty() { return "empty displayname".into_response(); }
    let username = if form.username.trim().is_empty() { slug::slug_from(&displayname) } else { form.username };
    if displayname.len() > 255 || username.len() > 255 || form.password.len() > 255 { return "displayname, username or password are too long".into_response(); }
    if !slug::slug_valid(&username) {
        return "invalid username".into_response();
    }
    if (form.week_len != 7 && form.week_len != 8) || form.logsday_weekday < 0 || form.logsday_weekday > 6 { return "no.".into_response(); }
    let logsday_weekday = if form.week_len == 7 { form.logsday_weekday } else { form.logsday_weekday + 1 };
    let result = db::create_user(&state, &username, &displayname, &form.password, form.week_len, logsday_weekday).await;
    match result {
        Ok(_) => {
            if let Ok(_) = tokio::fs::create_dir_all(format!("uploads/users/{}", username)).await {
                let u = db::get_user_by_username(&state, &username).await;
                if let None = u { return (StatusCode::INTERNAL_SERVER_ERROR, "Couldn't find user after creating").into_response(); }
                let uid = u.unwrap().uid;
                session.insert("uid", uid).await.unwrap();
                return hx_redirect("/project").into_response();
            } else {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Could not create user directory").into_response();
            }
        }
        Err(e) => {
            println!("{}", e);
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
            return hx_redirect("/project").into_response();
        }
    }
    return "Incorrect Username or Password".into_response();
}

// Route /logout

async fn get_logout(session: Session) -> impl IntoResponse {
    match session.remove::<i64>("uid").await {
        Ok(_) => { return Redirect::to("/").into_response(); }
        Err(e) => { println!("{}", e); return Html(MessageTemplate{message: "You're not logged in.".into()}.render().unwrap()).into_response(); }
    }
}

// Route /project
#[derive(Template)]
#[template(path = "projectlist.html")]
struct ProjectListTemplate {
    projects: Vec<Project>,
    user: User,
}

async fn get_project_list(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    let user_id = session.get::<i64>("uid").await.unwrap();
    if let None = user_id {
        return Redirect::to("/login").into_response();
    }
    let user_id = user_id.unwrap();
    let user = db::get_user(&state, user_id).await.unwrap();
    let projects = db::get_user_projects(&state, user_id).await;
    let render = ProjectListTemplate{projects, user}.render();
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
    let user_id = session.get::<i64>("uid").await.unwrap();
    match user_id {
        Some(uid) => {
            let project = db::get_project_by_slug(&state, uid, &project_slug).await;
            if let None = project { return "Project not found".into_response(); }
            let project = project.unwrap();

            let u = db::get_user(&state, uid).await;
            if let None = u { return redirect_login().into_response(); }
            let username = u.unwrap().username;

            let logs = db::get_project_logs(&state, project.uid).await;
            let render = EditProjectTemplate{username, project, logs}.render();
            if let Ok(render) = render {
                return Html(render).into_response();
            }
            return generic_error().into_response();
        },
        None => redirect_login().into_response()
    }
}

// Route /project/{project_slug}/{log_number}
#[derive(Template)]
#[template(path = "editlog.html")]
struct EditLogTemplate {
    username: String,
    project: Project,
    log: LogEntry,
}

async fn get_edit_log(session: Session, State(state): State<AppState>, Path((project_slug, log_number)): Path<(String, String)>) -> impl IntoResponse {
    let user_id: Option<i64> = session.get("uid").await.unwrap();
    match user_id {
        Some(uid) => {
            let user = db::get_user(&state, uid).await;
            if let None = user { return "could not get user data".into_response(); }
            let user = user.unwrap();

            let project = db::get_project_by_slug(&state, uid, &project_slug).await.unwrap();

            let log_number = log_number.parse::<i64>();
            if let Err(e) = log_number { return e.to_string().into_response(); }
            let log_number = log_number.unwrap();

            let log = db::get_log_uuid_pslug_lslug(&state, uid, &project_slug, log_number).await;
            if let None = log { return "Log not found".into_response(); }
            let log = log.unwrap();

            let render = EditLogTemplate{username: user.username, project, log}.render();
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
    project: Project,
    md_content: String, // leave empty ("") for empty md_content
}

async fn get_new_log(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>) -> impl IntoResponse {
    let uid = if let Some(uid) = session.get::<i64>("uid").await.unwrap() { uid } else { return redirect_login().into_response(); };
    let user = if let Some(u) = db::get_user(&state, uid).await { u } else { return redirect_login().into_response(); };
    match newlog::newlog_num(&state, &user, &project_slug).await {
        NewlogResult::New(project, _) => { return Html(NewLogTemplate{project, md_content: "".into()}.render().unwrap()).into_response(); },
        NewlogResult::Edit(project, log) => {
            let md_content = match fs::read_to_string(format!("uploads/users/{}/{}/{}/index.md", &user.username, &project.slug, log.number)) { Ok(s) => s, Err(e) => { println!("{:?}", e); return generic_error().into_response(); } };
            return Html(NewLogTemplate{project, md_content}.render().unwrap()).into_response();
        },
        NewlogResult::NotLogsday => { return msg_html("Not your Logsday! Go touch some logs!".into()).into_response(); },
        NewlogResult::AlreadyUploadedForProject { project_uid: _ } => { return msg_html("You've already uploaded a log this week for a different project! Go touch some logs and come back next week!".into()).into_response(); },
        NewlogResult::ProjectNotFound => { return msg_html("Project doesn't exist!".into()).into_response(); }
    }
}

#[derive(Deserialize)]
struct NewLogRequest {
    title: String,
    content: String,
}

async fn post_new_log(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>, Form(form): Form<NewLogRequest>,) -> impl IntoResponse {
    if form.title.len() > 255 { return "title too long".into_response(); }
    if form.content.as_bytes().len() > 1024 * 1024 { return "why do you have 1MB of text..?".into_response(); }
    let uid = if let Some(uid) = session.get::<i64>("uid").await.unwrap() { uid } else { return hx_redirect("/login").into_response(); };
    let user = if let Some(u) = db::get_user(&state, uid).await { u } else { return hx_redirect("/login").into_response(); };
    match newlog::newlog_num(&state, &user, &project_slug).await {
        NewlogResult::Edit(project, LogEntry { number: log_number, .. }) |
        NewlogResult::New(project, log_number) => {
            let log_path = format!("uploads/users/{}/{}/{}", &user.username, &project_slug, &log_number);
            let log_content_path = format!("{}/{}", &log_path, "index.md");
            let log_content_rendered_path = format!("{}/{}", &log_path, "index.html");
            match db::create_log(&state, project.uid, &form.title, log_number).await {
                Ok(_) => {
                    if let Err(e) = tokio::fs::create_dir_all(log_path).await { println!("{}", e); return "couldn't create log dir".into_response(); }
                    let html_render = filestuff::render_markdown_to_html(&form.content);
                    if let Err(e) = fs::write(log_content_path, &form.content) { println!("{}", e); return "couldn't write content".into_response(); }
                    if let Err(e) = fs::write(log_content_rendered_path, &html_render) { println!("{}", e); return "couldn't write rendered content".into_response(); }
                    return hx_redirect(&format!("/project/{}", project_slug)).into_response();
                },
                Err(e) => {
                    println!("{} -- project/uid = '{}'/{}, log # = {}", e, project.title, project.uid, log_number);
                    return "Database Error".into_response();
                }
            }
        },
        NewlogResult::NotLogsday => { return "Not your Logsday! Go touch some logs!".into_response(); },
        NewlogResult::AlreadyUploadedForProject { project_uid: _ } => { return "You've already uploaded a log this week for a different project! Go touch some logs and come back next week!".into_response(); },
        NewlogResult::ProjectNotFound => { return "Project doesn't exist!".into_response(); }
    }
}

// Route /del/project/{project_slug}

async fn post_del_project(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>) -> impl IntoResponse {
    let uid = if let Some(uid) = session.get::<i64>("uid").await.unwrap() { uid } else { return hx_redirect("/login").into_response(); };
    let u = if let Some(u) = db::get_user(&state, uid).await { u } else { return hx_redirect("/login").into_response(); };
    let project = if let Some(p) = db::get_project_by_slug(&state, uid, &project_slug).await { p } else { return "Project not found".into_response(); };
    if db::delete_project(&state, project.uid).await {
        if let Err(e) = tokio::fs::remove_dir_all(format!("uploads/users/{}/{}", u.username, &project_slug)).await {
            return e.to_string().into_response();
        }
        return hx_redirect("/project").into_response();
    }
    return "Project does not exist or cannot be deleted".into_response();
}

// Route /del/log/{project_slug}/{log_number}

async fn post_del_log(session: Session, State(state): State<AppState>, Path((project_slug, log_number)): Path<(String,String)>) -> impl IntoResponse {
    let uid = if let Some(uid) = session.get::<i64>("uid").await.unwrap() { uid } else { return hx_redirect("/login").into_response(); };
    let u = if let Some(u) = db::get_user(&state, uid).await { u } else { return hx_redirect("/login").into_response(); };
    let project = if let Some(p) = db::get_project_by_slug(&state, uid, &project_slug).await { p } else { return "Project not found".into_response(); };
    let log_number = match log_number.parse::<i64>() { Ok(num) => num, Err(e) => return e.to_string().into_response() };
    let log = if let Some(log) = db::get_log_by_slug(&state, project.uid, log_number).await { log } else { return "Log not found".into_response(); };
    if db::delete_log(&state, log.uid).await {
        if let Err(e) = tokio::fs::remove_dir_all(format!("uploads/users/{}/{}/{}", u.username, &project_slug, log_number)).await {
            return e.to_string().into_response();
        }
        return hx_redirect(&format!("/project/{}", project_slug)).into_response();
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
    let u = if let Some(u) = db::get_user_by_username(&state, &username).await { u } else { return Html(MessageTemplate{message: "User does not exist".to_string()}.render().unwrap()).into_response(); };
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
    let u = if let Some(u) = db::get_user_by_username(&state, &username).await { u } else { return Html(MessageTemplate{message: "User does not exist".to_string()}.render().unwrap()).into_response(); };
    let project = if let Some(p) = db::get_project_by_slug(&state, u.uid, &project_slug).await { p } else { return "Project not found".into_response(); };
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
    let u = if let Some(u) = db::get_user_by_username(&state, &username).await { u } else { return Html(MessageTemplate{message: "User does not exist".to_string()}.render().unwrap()).into_response(); };
    let project = if let Some(p) = db::get_project_by_slug(&state, u.uid, &project_slug).await { p } else { return "Project not found".into_response(); };
    let log_number = match log_number.parse::<i64>() { Ok(num) => num, Err(e) => return e.to_string().into_response() };
    let log = if let Some(log) = db::get_log_by_slug(&state, project.uid, log_number).await { log } else { return "Log not found".into_response(); };
    return Html(ViewLogTemplate{owner: u, project, log}.render().unwrap()).into_response();
}

// Route /new/log/{project_slug}/upload

#[derive(TryFromMultipart)]
struct LogMediaUploadRequest {
    #[form_data(field_name = "file", limit = "100MB")]
    file: FieldData<Bytes>,
}

// return the json of the file data on success; return an error code that will be displayed with js on error
async fn post_new_log_media_upload(session: Session, State(state): State<AppState>, Path(project_slug): Path<String>, data: TypedMultipart<LogMediaUploadRequest>) -> impl IntoResponse {
    let uid = if let Some(uid) = session.get::<i64>("uid").await.unwrap() { uid } else { return "Not logged in".into_response(); };
    let user = if let Some(u) = db::get_user(&state, uid).await { u } else { return "Not logged in".into_response(); };
    match newlog::newlog_num(&state, &user, &project_slug).await {
        NewlogResult::Edit(_, LogEntry { number: log_number , .. }) |
        NewlogResult::New(_, log_number) => {
            let content_type = if let Some(t) = &data.file.metadata.content_type { t } else { return (StatusCode::INTERNAL_SERVER_ERROR, "Could not get file type").into_response(); };
            if filestuff::mime_media_type(&content_type) == MediaType::Unsupported { return (StatusCode::BAD_REQUEST, "unsupported file type").into_response(); }
            let file_name = data.file.metadata.file_name.as_ref().unwrap();
            if !filestuff::filename_valid(file_name) { return (StatusCode::BAD_REQUEST, "invalid filename").into_response(); }
            let file_name = filestuff::normalize_extension(file_name);
            if !filestuff::verify_magic_bytes_match_extension(&file_name, &data.file.contents).await { return (StatusCode::BAD_REQUEST, "file extension doesn't match contents").into_response(); }
            let log_path = format!("uploads/users/{}/{}/{}", &user.username, &project_slug, &log_number);
            let log_file_path = format!("{}/{}", &log_path, &file_name);
            let log_file_web_path = format!("/uploads/{}/{}/{}/{}", &user.username, &project_slug, &log_number, &file_name);
            if let Err(e) = fs::create_dir_all(&log_path) { println!("{}", e); return (StatusCode::INTERNAL_SERVER_ERROR, "can't create log dir").into_response(); }
            let current_size = filestuff::get_directory_size_bytes(&log_path).await.unwrap_or(0);
            let incoming_size = data.file.contents.len() as u64;
            if current_size + incoming_size > 100 * 1024 * 1024 { return (StatusCode::INSUFFICIENT_STORAGE, "cannot upload more than 100MB per log").into_response(); }
            if let Err(e) = fs::write(log_file_path, &data.file.contents) { println!("{}", e); return (StatusCode::INTERNAL_SERVER_ERROR, "can't write file").into_response(); }
            return newlog::log_response(&file_name, incoming_size, &log_file_web_path).into_response();
        },
        NewlogResult::NotLogsday => { return (StatusCode::BAD_REQUEST, "Not Logsday").into_response(); },
        NewlogResult::AlreadyUploadedForProject { project_uid: _ } => { return (StatusCode::BAD_REQUEST, "This log was not created today").into_response(); },
        NewlogResult::ProjectNotFound => { return (StatusCode::BAD_REQUEST, "Project not found").into_response(); }
    }
}

// Route /new/log/{project_slug}/delete/{filename_to_delete}

async fn delete_log_media_delete(session: Session, State(state): State<AppState>, Path((project_slug, delete_filename)): Path<(String, String)>) -> impl IntoResponse {
    let uid = if let Some(uid) = session.get::<i64>("uid").await.unwrap() { uid } else { return hx_redirect("/login").into_response(); };
    let u = if let Some(u) = db::get_user(&state, uid).await { u } else { return hx_redirect("/login").into_response(); };
    if let None = db::get_project_by_slug(&state, uid, &project_slug).await { return (StatusCode::BAD_REQUEST, "Project not found").into_response(); }
    let log_number = db::get_last_project_log(&state, uid, &project_slug).await.unwrap_or_default().number + 1;
    let file_name = delete_filename.as_ref();
    if !filestuff::filename_valid(file_name) { return (StatusCode::BAD_REQUEST, "invalid filename").into_response(); }
    let file_name = filestuff::normalize_extension(file_name);
    let log_path = format!("uploads/users/{}/{}/{}", &u.username, &project_slug, &log_number);
    let log_file_path = format!("{}/{}", &log_path, &file_name);
    match fs::remove_file(&log_file_path) {
        Ok(_) => { return (StatusCode::OK, "File deleted successfully").into_response(); },
        Err(e) => {
            println!("Failed to delete file {}: {}", log_file_path, e);
            return (StatusCode::OK, "File already does not exist").into_response();
        }
    }
}

// Route /bits/nav-user

#[derive(Template)]
#[template(path = "bits/nav_user.html")]
struct NavUserBitTemplate {
    user: User
}

#[derive(Template)]
#[template(path = "bits/login.html")]
struct LoginBitTemplate;

async fn get_nav_user_bit(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    let uid = session.get::<i64>("uid").await.unwrap();
    match uid {
        Some(uid) => {
            let user = if let Some(u) = db::get_user(&state, uid).await { u } else { return "error".into_response(); };
            return Html(NavUserBitTemplate{user}.render().unwrap()).into_response();
        },
        None => {
            return Html(LoginBitTemplate.render().unwrap()).into_response();
        }
    }
}