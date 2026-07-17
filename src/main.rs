use askama::Template;
use axum::{
    Form, Router, ServiceExt, body::Bytes, extract::{DefaultBodyLimit, FromRef, FromRequestParts}, http::{HeaderMap, StatusCode, header, request::Parts}, response::{Html, IntoResponse, Redirect, Response}, routing::{delete, get, post} //async_trait
};
use axum::extract::{Path, Query, State};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
// use tower_governor::{GovernorLayer, governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor};
use std::{collections::HashMap, fs, net::SocketAddr};
use tower_http::services::ServeDir;
use sqlx::sqlite::SqlitePool;
use serde::Deserialize;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::time};
use tower_sessions::Session;
use tower_http::normalize_path::NormalizePath;
use tokio_cron_scheduler::{Job, JobScheduler};
use tokio::sync::mpsc;

use crate::{filestuff::MediaType, newlog::{NewlogResult, error_json}};

mod db;
mod slug;
mod week;
mod filestuff;
mod newlog;
mod password;

const WEEKDAY_NAMES: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Satruday", "Sunday"];

macro_rules! get_or {
    ($expr:expr, $fallback:expr) => {
        match $expr {
            Some(val) => val,
            None => {
                return $fallback.into_response();
            }
        }
    };
    ($expr:expr, $fallback:expr, err) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                println!("-- {}: {e}", line!());
                return $fallback.into_response();
            }
        }
    };
}

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
    tx: mpsc::Sender<filestuff::CompressVideoJob>,
}

#[tokio::main]
async fn main() {
    let db_pool = SqlitePool::connect("sqlite:sqlite.db")
        .await
        .expect("Could not connect to database. Please create `sqlite.db` database.");

    let session_store = MemoryStore::default(); // store user sessions to memory for now
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false) // set to true later when have HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(1)));

    // prevent same IP from spamming requests; up to 250 requests in burst allowed, refreshing 1 every second
    // let governor_config = GovernorConfigBuilder::default()
    //     .key_extractor(SmartIpKeyExtractor)
    //     .per_second(1)
    //     .burst_size(250)
    //     .finish()
    //     .expect("Could not create governor_config");

    // let governor_limiter = governor_config.limiter().clone();

    // every minute, clean up old ips; prevents them from being stored indefinitely
    // tokio::spawn(async move {
    //     loop {
    //         tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    //         governor_limiter.retain_recent();
    //     }
    // });

    let (tx, mut rx) = mpsc::channel::<filestuff::CompressVideoJob>(100);
    let state = AppState { db: db_pool, tx };

    // Thread that compresses videos
    tokio::spawn(async move {
        while let Some(job) = rx.recv().await {
            let metadata = match fs::metadata(&job.path) {
                Ok(meta) => meta,
                Err(_) => { println!("COMPRESS WARN: file was deleted {}", job.path); continue; }
            };
            if let Ok(modified) = metadata.modified() {
                if modified > job.created_on { println!("COMPRESS WARN: file was updated {:?}", job.path); continue; }
            }
            println!("COMPRSS START: {}", job.path);
            filestuff::compress_video(job).await;
        }
        println!("Shutting down compression thread");
    });

    let app = Router::new()
        .route("/debug", get(get_debug))
        .route("/", get(landing))
        .route("/signup", get(get_signup).post(post_signup))
        .route("/login", get(get_login).post(post_login))
        .route("/logout", get(get_logout).post(post_logout))
        .route("/mdguide", get(get_mdguide))
        .route("/credits", get(get_credits))
        .route("/account", get(get_account))
        .route("/account/change-displayname", post(post_change_displayname))
        .route("/account/change-pfp", post(post_change_pfp))
        .route("/new/project", get(get_new_project).post(post_new_project))
        .route("/new/log/{project_slug}", get(get_new_log).post(post_new_log))
        .route("/edit/log/{project_slug}/{log_number}", get(get_edit_log).post(post_edit_log))
        .route("/new/media/{project_slug}", post(post_new_log_media_upload))
        .route("/new/media/{project_slug}/{log_number}", post(post_log_media_upload))
        .route("/del/user/{username}", post(post_del_user))
        .route("/del/project/{project_slug}", post(post_del_project))
        .route("/del/log/{project_slug}/{log_number}", post(post_del_log))
        .route("/del/media/{project_slug}/new/{delete_filename}", delete(delete_new_log_media))
        .route("/del/media/{project_slug}/{log_number}/{delete_filename}", delete(delete_log_media))
        .route("/comment/{username}/{project_slug}/{log_number}", get(get_log_comments).post(post_log_comments))
        .route("/u", get(get_view_self))
        .route("/u/{username}", get(get_view_user))
        .route("/u/{username}/{project_slug}", get(get_view_project))
        .route("/u/{username}/{project_slug}/{log_number}", get(get_view_log))
        .route("/bits/nav-user", get(get_nav_user_bit))
        .route("/like/{ty}/{uid}", get(get_like))
        .route("/like/{ty}/{uid}/{action}", post(post_like))
        .route("/favicon.ico", get(get_favicon))
        .nest_service("/uploads", ServeDir::new("uploads/users"))
        .nest_service("/static", ServeDir::new("static"))
        // .layer(GovernorLayer::new(governor_config))
        .layer(session_layer)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024 * 1024)) // do not allow uploads of over 10GB; should also be enforced on client side
        .with_state(state.clone());

    let app = NormalizePath::trim_trailing_slash(app.into_service());
    let app = app.into_make_service_with_connect_info::<SocketAddr>(); // let app = app.into_make_service();

    // set up the cleanup cron job
    let sched = JobScheduler::new().await.unwrap();
    let cleanup_job = Job::new_async("0 0 0 * * *", move |_uuid, _l| {
        println!("STARTED CLEANUP");
        let job_state = state.clone();
        Box::pin(async move {
            if let Err(e) = filestuff::cleanup_all_log_directories(job_state).await {
                println!("FAILED CLEANUP - {e}");
            } else {
                println!("FINISHED CLEANUP");
            }
        })
    })
    .unwrap();
    sched.add(cleanup_job).await.unwrap();
    sched.start().await.unwrap();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Serving to `http://localhost:3000`");
    axum::serve(listener, app).await.unwrap();
}

fn generic_error() -> impl IntoResponse {
    return Html("Oops, something went wrong... Go touch some logs in the meantime.").into_response();
}

fn hx_redirect(route: &str) -> impl IntoResponse {
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

impl LogEntry {
    /// note, does not check if user is admin
    pub fn can_edit(&self) -> bool {
        return week::days_since(self.created_on) == 0; // can edit if uploaded today
    }
}

#[derive(Debug, sqlx::FromRow, Default)]
struct Comment {
    displayname: String,
    username: String,
    text: String,
    created_on: week::UnixTime,
}

struct AuthdUser(User);

impl<S> FromRequestParts<S> for AuthdUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let session = parts.extensions.get::<Session>().ok_or_else(|| Redirect::to("/login").into_response())?;
        return AuthdUser::get_user(session, &app_state).await.ok_or(Redirect::to("/login").into_response());
    }
}

impl AuthdUser {
    async fn get_user(session: &Session, state: &AppState) -> Option<AuthdUser> {
        let uid = session.get::<i64>("uid").await.unwrap_or(None)?;
        match db::get_user(state, uid).await {
            Some(user) => Some(AuthdUser(user)),
            None => {
                let _ = session.clear().await; 
                return None;
            }
        }
    }
}

async fn _testing(Query(params): Query<HashMap<String, String>>, Path(user_id): Path<u32>) -> String {
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
    display_users: Vec<User>,
}

async fn landing(State(state): State<AppState>) -> impl IntoResponse {
    let display_users = db::get_all_users(&state).await;
    let render = LandingTemplate { display_users }.render();
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
    let password = get_or!(password::hash(&form.password), "Could not hash password; should not be possible", err);
    if !password::verify(&form.password, &password) { return "Could not verify password after hashing it; should not be possible".into_response(); }
    let result = db::create_user(&state, &username, &displayname, &password, form.week_len, logsday_weekday).await;
    match result {
        Ok(_) => {
            if let Ok(_) = fs::create_dir_all(format!("uploads/users/{}", username)) {
                let pfp_path = format!("uploads/users/{}/pfp.webp", &username);
                let default_pfp = get_or!(fs::read("static/favicon.ico"), (StatusCode::INTERNAL_SERVER_ERROR, "Could not find favicon"), err);
                let webp_img = get_or!(filestuff::convert_to_webp(&default_pfp), (StatusCode::INTERNAL_SERVER_ERROR, "Could not convert to webp"));
                if let Err(_) = fs::write(pfp_path, &webp_img) { return (StatusCode::INTERNAL_SERVER_ERROR, "Could not write file").into_response(); }
                let u = db::get_user_by_username(&state, &username).await;
                if let None = u { return (StatusCode::INTERNAL_SERVER_ERROR, "Couldn't find user after creating").into_response(); }
                let uid = u.unwrap().uid;
                session.insert("uid", uid).await.unwrap();
                return hx_redirect("/u").into_response();
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
        if password::verify(&form.password, &u.password) {
            session.insert("uid", u.uid).await.unwrap();
            return hx_redirect("/u").into_response();
        }
    }
    return "Incorrect Username or Password".into_response();
}

// Route /logout

async fn get_logout(session: Session) -> impl IntoResponse {
    match session.remove::<i64>("uid").await {
        Ok(_) => { return Redirect::to("/").into_response(); }
        Err(e) => { println!("{}", e); return msg_html("You're not logged in.".into()).into_response(); }
    }
}

async fn post_logout(session: Session) -> impl IntoResponse {
    let _ = session.remove::<i64>("uid").await;
    return (StatusCode::OK, [("HX-Refresh", "true")], "");
}

// Route /mdguide

#[derive(Template)]
#[template(path = "mdguide.html")]
struct MdGuideTemplate;

async fn get_mdguide() -> impl IntoResponse {
    return Html(MdGuideTemplate.render().unwrap()).into_response();
}

// Route /credits

#[derive(Template)]
#[template(path = "credits.html")]
struct CreditsTemplate;

async fn get_credits() -> impl IntoResponse {
    return Html(CreditsTemplate.render().unwrap()).into_response();
}

// Route /u

#[derive(Template)]
#[template(path = "viewuser.html")]
struct ViewUserTemplate {
    user: User,
    projects: Vec<Project>,
    owner: bool,
}

async fn get_view_self(AuthdUser(user): AuthdUser, State(state): State<AppState>) -> impl IntoResponse {
    let projects = db::get_user_projects(&state, user.uid).await;
    return Html(ViewUserTemplate{user, projects, owner: true}.render().unwrap()).into_response();
}

// Route /u/{username}

async fn get_view_user(session: Session, State(state): State<AppState>, Path(username): Path<String>) -> impl IntoResponse {
    let authd_uid = match AuthdUser::get_user(&session, &state).await { Some(u) => u.0.uid, None => 0 }; // no user will have uid of 0; ever
    let user = get_or!(db::get_user_by_username(&state, &username).await, msg_html("User does not exist".into()));
    let projects = db::get_user_projects(&state, user.uid).await;
    let owner = authd_uid == user.uid;
    return Html(ViewUserTemplate{user, projects, owner}.render().unwrap()).into_response();
}

// Route /u/{username}/{project_slug}

#[derive(Template)]
#[template(path = "viewproject.html")]
struct ViewProjectTemplate {
    user: User,
    project: Project,
    logs: Vec<LogEntry>,
    owner: bool,
}

async fn get_view_project(session: Session, State(state): State<AppState>, Path((username, project_slug)): Path<(String, String)>) -> impl IntoResponse {
    let authd_uid = match AuthdUser::get_user(&session, &state).await { Some(u) => u.0.uid, None => 0 }; // no user will have uid of 0; ever
    let user = get_or!(db::get_user_by_username(&state, &username).await, msg_html("User does not exist".into()));
    let project = get_or!(db::get_project_by_slug(&state, user.uid, &project_slug).await, msg_html("Project does not exist".into()));
    let logs = db::get_project_logs(&state, project.uid).await;
    let owner = authd_uid == user.uid;
    return Html(ViewProjectTemplate{user, project, logs, owner}.render().unwrap()).into_response();
}

// Route /u/{username}/{project_slug}/{log_number}

#[derive(Template)]
#[template(path = "viewlog.html")]
struct ViewLogTemplate {
    user: User,
    project: Project,
    log: LogEntry,
    owner: bool,
    authd: bool,
}

async fn get_view_log(session: Session, State(state): State<AppState>, Path((username, project_slug, log_number)): Path<(String, String, i64)>) -> impl IntoResponse {
    let authd_uid = match AuthdUser::get_user(&session, &state).await { Some(u) => u.0.uid, None => 0 }; // no user will have uid of 0; ever
    let user = get_or!(db::get_user_by_username(&state, &username).await, msg_html("User does not exist".into()));
    let project = get_or!(db::get_project_by_slug(&state, user.uid, &project_slug).await, msg_html("Project does not exist".into()));
    let log = get_or!(db::get_log_by_number(&state, project.uid, log_number).await, msg_html("Log does not exist".into()));
    let owner = authd_uid == user.uid;
    let authd = authd_uid != 0;
    return Html(ViewLogTemplate{user, project, log, owner, authd}.render().unwrap()).into_response();
}

// Route /new/project

#[derive(Template)]
#[template(path = "newproject.html")]
struct NewProjectTemplate;

async fn get_new_project(AuthdUser(_): AuthdUser) -> impl IntoResponse {
    return Html(NewProjectTemplate.render().unwrap()).into_response();
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

async fn post_new_project(State(state): State<AppState>, AuthdUser(user): AuthdUser, data: TypedMultipart<NewProjectRequest>) -> impl IntoResponse {
    let pslug: &str = if data.slug.len() == 0 { &slug::slug_from(&data.title) } else { &data.slug };

    if data.title.len() > 255 || pslug.len() > 255 { return "title or slug too long".into_response(); }
    if data.title.len() == 0 || pslug.len() == 0 { return "title or slug is empty".into_response(); }
    if data.description.len() > 65535 { return "description too long".into_response(); }

    let (thumbnail, content_type) = if data.thumbnail.contents.len() == 0 {
        (&fs::read("static/favicon.ico").unwrap()[..], "image/x-icon")
    } else {
        (&data.thumbnail.contents[..], data.thumbnail.metadata.content_type.as_ref().unwrap() as &str)
    };

    if filestuff::mime_media_type(content_type) != MediaType::Image { return "Unsupported thumbnail file format".into_response(); }
    if !slug::slug_valid(&pslug) { return "Project slug is invalid".into_response(); }
    let project_path = format!("uploads/users/{}/{}", &user.username, &pslug);
    let thumbnail_path = format!("{}/{}", &project_path, "thumb.webp");

    let webp_img = filestuff::convert_to_webp(thumbnail);
    if let None = webp_img { return "Could not convert to webp".into_response(); }
    let webp_img = webp_img.unwrap();

    if let Ok(_) = db::create_project(&state, user.uid, &data.title, &pslug, &data.description).await {
        if let Ok(_) = fs::create_dir_all(project_path) {
            if let Ok(_) = fs::write(thumbnail_path, &webp_img) {
                return hx_redirect("/u").into_response();
            }
        }
    }
    return generic_error().into_response();
}

// Route /new/log/{project_slug}

#[derive(Template)]
#[template(path = "newlog.html")]
struct UploadLogTemplate {
    user: User,
    project: Project,
    title: String,
    md: String,
    upload_path: String,
    files_json_list: String,
    exists: bool,
    log_num: i64,
}

impl UploadLogTemplate {
    fn new(user: User, project: Project, log_num: i64) -> UploadLogTemplate {
        let files_json_list = serde_json::to_string(&newlog::get_existing_files(&user, &project, log_num)).unwrap_or("[]".into());
        let upload_path = format!("/new/log/{}", &project.slug);
        UploadLogTemplate { user, project, title: "".into(), md: "".into(), upload_path, files_json_list, exists: false, log_num }
    }
}

async fn get_new_log(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path(project_slug): Path<String>) -> impl IntoResponse {
    match newlog::newlog_num(&state, &user, &project_slug).await {
        NewlogResult::New(project, num) => { return Html(UploadLogTemplate::new(user, project, num).render().unwrap()).into_response(); },
        NewlogResult::NotLogsday => { return msg_html("Not your Logsday! Go touch some logs!".into()).into_response(); },
        NewlogResult::AlreadyUploaded => { return msg_html("You've already uploaded a log this week! Go touch some logs and come back next week!".into()).into_response(); },
        NewlogResult::ProjectNotFound => { return msg_html("Project doesn't exist!".into()).into_response(); }
    }
}

#[derive(Deserialize)]
struct NewLogRequest {
    title: String,
    content: String,
}

async fn post_new_log(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path(project_slug): Path<String>, Form(form): Form<NewLogRequest>,) -> impl IntoResponse {
    if form.title.len() > 255 { return "title too long".into_response(); }
    if form.content.as_bytes().len() > 1024 * 1024 { return "why do you have 1MB of text..?".into_response(); }
    match newlog::newlog_num(&state, &user, &project_slug).await {
        NewlogResult::New(project, log_number) => {
            let log_path = format!("uploads/users/{}/{}/{}", &user.username, &project_slug, &log_number);
            let log_content_path = format!("{}/{}", &log_path, "index.md");
            let log_content_rendered_path = format!("{}/{}", &log_path, "index.html");
            let html_render = filestuff::render_markdown_to_html(&form.content);
            let linked_size = get_or!(filestuff::count_log_directory_size(&log_path, &html_render), "Something went wrong when looking at embedded files", err);
            if linked_size > 1024 * 1024 * 1024 { return "Your log must be smaller than 1GB".into_response(); }
            match db::create_log(&state, project.uid, &form.title, log_number).await {
                Ok(_) => {
                    if let Err(e) = fs::create_dir_all(&log_path) { println!("{}", e); return "Couldn't create log dir".into_response(); }
                    if let Err(e) = fs::write(log_content_path, &form.content) { println!("{}", e); return "Couldn't write content".into_response(); }
                    if let Err(e) = fs::write(log_content_rendered_path, &html_render) { println!("{}", e); return "Couldn't write rendered content".into_response(); }
                    if let Err(e) = filestuff::cleanup_log_directory(&log_path, &state).await { println!("couldn't clean up: {}", e); }
                    return hx_redirect(&format!("/u/{}/{}", user.username, project_slug)).into_response();
                },
                Err(e) => {
                    println!("{} -- project/uid = '{}'/{}, log # = {}", e, project.title, project.uid, log_number);
                    return "Database Error".into_response();
                }
            }
        },
        NewlogResult::NotLogsday => { return "Not your Logsday! Go touch some logs!".into_response(); },
        NewlogResult::AlreadyUploaded => { return "You've already uploaded a log this week! Go touch some logs and come back next week!".into_response(); },
        NewlogResult::ProjectNotFound => { return "Project doesn't exist! Check the URL for misspelled project slug you silly.".into_response(); }
    }
}

// Route /edit/log/{project_slug}/{log_number}

async fn get_edit_log(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path((project_slug, log_num)): Path<(String, i64)>) -> impl IntoResponse {
    let project = get_or!(db::get_project_by_slug(&state, user.uid, &project_slug).await, "Project does not exist");
    let log = get_or!(db::get_log_by_number(&state, project.uid, log_num).await, "Log does not exist");
    let md = get_or!(fs::read_to_string(format!("uploads/users/{}/{}/{}/index.md", &user.username, &project_slug, log_num)), "Cannot find log md file", err);
    // Can only edit today's logs
    if !user.admin && !log.can_edit() {
        return msg_html("You can only edit logs you created today.".into()).into_response();
    }
    let files_json_list = serde_json::to_string(&newlog::get_existing_files(&user, &project, log_num)).unwrap_or("[]".into());
    let upload_path = format!("/edit/log/{}/{}", &project.slug, log_num);
    return Html(UploadLogTemplate{user, project, title: log.title, md, upload_path, files_json_list, exists: true, log_num}.render().unwrap()).into_response();
}

async fn post_edit_log(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path((project_slug, log_number)): Path<(String, i64)>, Form(form): Form<NewLogRequest>,) -> impl IntoResponse {
    if form.title.len() > 255 { return "title too long".into_response(); }
    if form.content.as_bytes().len() > 1024 * 1024 { return "why do you have 1MB of text..?".into_response(); }
    let log = get_or!(db::get_log_uuid_pslug_lslug(&state, user.uid, &project_slug, log_number).await, "Log does not exist");
    let log_path = format!("uploads/users/{}/{}/{}", &user.username, &project_slug, &log_number);
    let log_content_path = format!("{}/{}", &log_path, "index.md");
    let log_content_rendered_path = format!("{}/{}", &log_path, "index.html");
    let html_render = filestuff::render_markdown_to_html(&form.content);
    let linked_size = get_or!(filestuff::count_log_directory_size(&log_path, &html_render), "Something went wrong when looking at embedded files", err);
    if linked_size > 1024 * 1024 * 1024 { return "Your log must be smaller than 1GB".into_response(); }
    if let Err(e) = db::update_log(&state, log.uid, &form.title).await { println!("{}", e); return "Database error".into_response(); }
    // the log must already exist; no need to re-create the log path
    if let Err(e) = fs::write(log_content_path, &form.content) { println!("{}", e); return "Couldn't write content".into_response(); }
    if let Err(e) = fs::write(log_content_rendered_path, &html_render) { println!("{}", e); return "Couldn't write rendered content".into_response(); }
    if let Err(e) = filestuff::cleanup_log_directory(&log_path, &state).await { println!("{}", e); }
    return hx_redirect(&format!("/u/{}/{}", user.username, project_slug)).into_response();
}

// Route /del/user/{username}

async fn post_del_user(session: Session, AuthdUser(user): AuthdUser, State(state): State<AppState>, Path(username): Path<String>) -> impl IntoResponse {
    if user.username != username { return "You can only delete account you're logged in to.".into_response(); }
    if !db::delete_user(&state, user.uid).await {
        return "Could not delete user".into_response();
    }
    if let Err(e) = fs::remove_dir_all(format!("uploads/users/{}", user.username)) {
        println!("{}", e);
        return "Could not clean up user directory after deletion".into_response();
    }
    let _ = session.remove::<i64>("uid").await;
    return (StatusCode::OK, [("HX-Refresh", "true")], "").into_response();
}

// Route /del/project/{project_slug}

async fn post_del_project(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path(project_slug): Path<String>) -> impl IntoResponse {
    let project = get_or!(db::get_project_by_slug(&state, user.uid, &project_slug).await, "Project does not exist");
    if !db::delete_project(&state, project.uid).await {
        return "Project does not exist or cannot be deleted".into_response();
    }
    if let Err(e) = fs::remove_dir_all(format!("uploads/users/{}/{}", user.username, &project_slug)) {
        println!("{}", e);
        return "Could not clean up uploads directory after deletion".into_response();
    }
    return hx_redirect("/u").into_response();
}

// Route /del/log/{project_slug}/{log_number}

async fn post_del_log(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path((project_slug, log_number)): Path<(String,i64)>) -> impl IntoResponse {
    let project = get_or!(db::get_project_by_slug(&state, user.uid, &project_slug).await, "Project does not exist");
    let log = get_or!(db::get_log_by_number(&state, project.uid, log_number).await, "Log does not exist");
    if !db::delete_log(&state, log.uid).await {
        return "Log does not exist or cannot be deleted".into_response();
    }
    if let Err(e) = fs::remove_dir_all(format!("uploads/users/{}/{}/{}", user.username, &project_slug, log_number)) {
        println!("{}", e);
        return "Could not clean up uploads directory after deletion".into_response();
    }
    return hx_redirect(&format!("/u/{}/{}", user.username, project_slug)).into_response();
}

// Route /new/media/{project_slug}

#[derive(TryFromMultipart)]
struct LogMediaUploadRequest {
    #[form_data(field_name = "file", limit = "1GB")]
    file: FieldData<Bytes>,
}

async fn post_new_log_media_upload(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path(project_slug): Path<String>, data: TypedMultipart<LogMediaUploadRequest>) -> impl IntoResponse {
    let project = get_or!(db::get_project_by_slug(&state, user.uid, &project_slug).await, error_json("Project does not exist"));
    let newlog_number = db::get_last_project_log_by_slug(&state, user.uid, &project_slug).await.unwrap_or_default().number + 1;
    return handle_upload(&user, &project, newlog_number, &data).await.into_response();
}

// Route /new/media/{project_slug}/{log_number}

/// return the json of the file data on success; return an error code that will be displayed with js on error
async fn post_log_media_upload(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path((project_slug, log_number)): Path<(String, i64)>, data: TypedMultipart<LogMediaUploadRequest>) -> impl IntoResponse {
    let project = get_or!(db::get_project_by_slug(&state, user.uid, &project_slug).await, error_json("Project does not exist"));
    let last_log_number = db::get_last_project_log_by_slug(&state, user.uid, &project_slug).await.unwrap_or_default().number + 1;
    if log_number > last_log_number { return error_json("Can only upload to existing log").into_response(); }
    return handle_upload(&user, &project, log_number, &data).await.into_response();
}

async fn handle_upload(user: &User, project: &Project, log_num: i64, data: &TypedMultipart<LogMediaUploadRequest>) -> impl IntoResponse {
    if !user.admin && !week::is_logsday(user.week_len, user.logsday_weekday) { return error_json("Not logsday").into_response(); }
    let content_type = get_or!(&data.file.metadata.content_type, (StatusCode::INTERNAL_SERVER_ERROR, error_json("Could not get file type")));
    if filestuff::mime_media_type(&content_type) == MediaType::Unsupported { return (StatusCode::BAD_REQUEST, error_json("Unsupported file type")).into_response(); }
    let file_name = get_or!(data.file.metadata.file_name.as_ref(), (StatusCode::BAD_REQUEST, error_json("Could not get file name")));
    if !filestuff::filename_valid(file_name) { return (StatusCode::BAD_REQUEST, error_json("Invalid filename")).into_response(); }
    let file_name = filestuff::normalize_extension(file_name);
    if !filestuff::verify_magic_bytes_match_extension(&file_name, &data.file.contents).await { return (StatusCode::BAD_REQUEST, error_json("file extension doesn't match contents")).into_response(); }
    let log_path = format!("uploads/users/{}/{}/{}", &user.username, &project.slug, &log_num);
    let log_file_path = format!("{}/{}", &log_path, &file_name);
    let log_file_web_path = format!("/uploads/{}/{}/{}/{}", &user.username, &project.slug, &log_num, &file_name);
    if let Err(e) = fs::create_dir_all(&log_path) { println!("{}", e); return (StatusCode::INTERNAL_SERVER_ERROR, error_json("can't create log dir")).into_response(); }
    let current_size = filestuff::get_directory_size_bytes(&log_path).await.unwrap_or(0);
    let incoming_size = data.file.contents.len() as u64;
    if current_size + incoming_size > 5 * 1024 * 1024 * 1024 { return (StatusCode::INSUFFICIENT_STORAGE, error_json("Cannot upload more than 5GB per log")).into_response(); }
    if let Err(e) = fs::write(log_file_path, &data.file.contents) { println!("{}", e); return (StatusCode::INTERNAL_SERVER_ERROR, error_json("can't write file")).into_response(); }
    return newlog::file_response(&file_name, incoming_size, &log_file_web_path).into_response();
}

// Route /del/media/{project_slug}/new/{delete_filename}

async fn delete_new_log_media(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path((project_slug, delete_filename)): Path<(String, String)>) -> impl IntoResponse {
    if let None = db::get_project_by_slug(&state, user.uid, &project_slug).await { return (StatusCode::BAD_REQUEST, "Project not found").into_response(); }
    let log_number = db::get_last_project_log_by_slug(&state, user.uid, &project_slug).await.unwrap_or_default().number + 1;
    let file_name = delete_filename.as_ref();
    if !filestuff::filename_valid(file_name) { return (StatusCode::BAD_REQUEST, "invalid filename").into_response(); }
    let file_name = filestuff::normalize_extension(file_name);
    let log_path = format!("uploads/users/{}/{}/{}", &user.username, &project_slug, &log_number);
    let log_file_path = format!("{}/{}", &log_path, &file_name);
    match fs::remove_file(&log_file_path) {
        Ok(_) => { return (StatusCode::OK, "File deleted successfully").into_response(); },
        Err(e) => {
            println!("Failed to delete file {}: {}", log_file_path, e);
            return (StatusCode::OK, "File already does not exist").into_response();
        }
    }
}

// Route /del/media/{project_slug}/{log_number}/{delete_filename}

async fn delete_log_media(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path((project_slug, log_number, delete_filename)): Path<(String, i64, String)>) -> impl IntoResponse {
    if let None = db::get_project_by_slug(&state, user.uid, &project_slug).await { return (StatusCode::BAD_REQUEST, "Project not found").into_response(); }
    if log_number != db::get_last_project_log_by_slug(&state, user.uid, &project_slug).await.unwrap_or_default().number + 1 { return "Only allowed to delete for today's log".into_response(); }
    let file_name = delete_filename.as_ref();
    if !filestuff::filename_valid(file_name) { return (StatusCode::BAD_REQUEST, "invalid filename").into_response(); }
    let file_name = filestuff::normalize_extension(file_name);
    let log_path = format!("uploads/users/{}/{}/{}", &user.username, &project_slug, &log_number);
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
    user: User,
    edit_log_info: Option<(String, i64)>,
}

#[derive(Template)]
#[template(path = "bits/login.html")]
struct LoginBitTemplate;

async fn get_nav_user_bit(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    let uid = session.get::<i64>("uid").await.unwrap();
    match uid {
        Some(uid) => {
            let user = if let Some(u) = db::get_user(&state, uid).await { u } else { return "error".into_response(); };
            let edit_log_info = match db::get_last_log(&state, user.uid).await {
                Some(log) => {
                    if week::days_since(log.created_on) == 0 {
                        Some((
                            db::get_project(&state, log.project_uid).await.unwrap().slug,
                            log.number
                        ))
                    } else {
                        None
                    }
                },
                None => None
            };
            return Html(NavUserBitTemplate{user, edit_log_info}.render().unwrap()).into_response();
        },
        None => {
            return Html(LoginBitTemplate.render().unwrap()).into_response();
        }
    }
}

// Route /comment/{username}/{project_slug}/{log_number}

#[derive(Template)]
#[template(path = "bits/comment_list.html")]
struct CommentListTemplate {
    comments: Vec<Comment>,
}

async fn get_log_comments(State(state): State<AppState>, Path((username, project_slug, log_number)): Path<(String, String, i64)>) -> impl IntoResponse {
    let owner = get_or!(db::get_user_by_username(&state, &username).await, "User does not exist");
    let log = get_or!(db::get_log_uuid_pslug_lslug(&state, owner.uid, &project_slug, log_number).await, "Log does not exist");
    let comments = db::get_comments_for_log(&state, log.uid).await;
    return Html(CommentListTemplate{comments}.render().unwrap()).into_response();
}

#[derive(Deserialize, Debug)]
struct PostCommentSubmission {
    text: String,
}

async fn post_log_comments(
    AuthdUser(user): AuthdUser, 
    State(state): State<AppState>,
    Path((username, project_slug, log_number)): Path<(String, String, i64)>,
    Form(form): Form<PostCommentSubmission>,
) -> impl IntoResponse {
    let owner = get_or!(db::get_user_by_username(&state, &username).await, "User does not exist");
    let log = get_or!(db::get_log_uuid_pslug_lslug(&state, owner.uid, &project_slug, log_number).await, "Log does not exist");
    if form.text.len() > 1024*1024 { return "Why is your comment a MEGABYTE long?".into_response(); }
    let res = db::create_comment_for_log(&state, log.uid, user.uid, &form.text).await;
    match res {
        Ok(_) => return (StatusCode::OK, [("HX-Trigger", "refreshComments")], "").into_response(),
        Err(e) => { println!("DB ERROR WHEN CREATING A COMMENT: {e}"); return (StatusCode::INTERNAL_SERVER_ERROR, "Could not create comment").into_response() },
    }
}

// Route /account

#[derive(Template)]
#[template(path = "account.html")]
struct AccountTemplate {
    user: User
}

async fn get_account(AuthdUser(user): AuthdUser) -> impl IntoResponse {
    return Html(AccountTemplate{user}.render().unwrap()).into_response();
}

#[derive(Deserialize, Debug)]
struct ChangeDisplaynameSubmission {
    displayname: String,
}

async fn post_change_displayname(AuthdUser(user): AuthdUser, State(state): State<AppState>, Form(form): Form<ChangeDisplaynameSubmission>) -> impl IntoResponse {
    if !db::update_user_displayname(&state, user.uid, &form.displayname).await {
        return "Database failure".into_response();
    }
    return (StatusCode::OK, [("HX-Refresh", "true")], "").into_response();
}

#[derive(TryFromMultipart)]
struct UpdatePfpRequest {
    #[form_data(field_name = "pfp", limit = "100MB")]
    pfp: FieldData<Bytes>,
}

async fn post_change_pfp(AuthdUser(user): AuthdUser, data: TypedMultipart<UpdatePfpRequest>) -> impl IntoResponse {
    if data.pfp.contents.len() == 0 { return "You did not upload a file.".into_response(); }
    let content_type = data.pfp.metadata.content_type.as_ref().unwrap();
    if filestuff::mime_media_type(content_type) != MediaType::Image { return "Unsupported profile picture file format".into_response(); }
    let user_path = format!("uploads/users/{}", &user.username);
    let pfp_path = format!("{}/{}", &user_path, "pfp.webp");

    let webp_img = get_or!(filestuff::convert_to_webp(&data.pfp.contents), "Could not convert to webp");
    if let Ok(_) = fs::create_dir_all(user_path) {
        if let Ok(_) = fs::write(pfp_path, &webp_img) {
            return (StatusCode::OK, [("HX-Refresh", "true")], "").into_response();
        } else {
            return "Could not write file".into_response();
        }
    } else {
        return "Could not create user directory".into_response();
    }
}

// Route /like/log/{log_uid}

#[derive(Template)]
#[template(path = "bits/likes.html")]
struct LikesTemplate {
    ty: String,
    uid: i64,
    like: Option<db::Like>,
    likes: db::Likes,
    authd: bool,
}

async fn get_like(session: Session, State(state): State<AppState>, Path((ty, uid)): Path<(String, i64)>) -> impl IntoResponse {
    let likes = match ty.as_ref() {
        "user" => db::get_user_likes(&state, uid).await,
        "project" => db::get_project_likes(&state, uid).await,
        "log" => db::get_log_likes(&state, uid).await,
        _ => { return "Wrong type".into_response(); }
    };
    let user_uid = session.get::<i64>("uid").await.unwrap_or(None);
    match user_uid {
        Some(user_uid) => {
            match db::get_user(&state, user_uid).await {
                Some(_) => {
                    let like = match ty.as_ref() {
                        "user" => db::get_user_like(&state, user_uid, uid).await,
                        "project" => db::get_project_like(&state, user_uid, uid).await,
                        "log" => db::get_log_like(&state, user_uid, uid).await,
                        _ => { return "Wrong type".into_response(); }
                    };
                    return Html(LikesTemplate{ty,uid,like,likes,authd:true}.render().unwrap()).into_response();
                }
                None => {
                    let _ = session.clear().await; 
                    return hx_redirect("/login").into_response();
                }
            }
        }
        None => {
            return Html(LikesTemplate{ty,uid,like:None,likes,authd:false}.render().unwrap()).into_response();
        }
    }
}

// Route /like/{log|project|user}/{uid}/{like|dislike|unlike}

async fn post_like(AuthdUser(user): AuthdUser, State(state): State<AppState>, Path((ty, uid, action)): Path<(String, i64, String)>) -> impl IntoResponse {
    match ty.as_ref() {
        "log" => {
            match action.as_ref() {
                "like" => { if let Err(e) = db::set_log_like(&state, user.uid, uid, Some(db::Like{is_like:true})).await { println!("DB Error when trying to like: {e}")} }
                "dislike" => { if let Err(e) = db::set_log_like(&state, user.uid, uid, Some(db::Like{is_like:false})).await { println!("DB Error when trying to dislike: {e}")} }
                "unlike" => { if let Err(e) = db::set_log_like(&state, user.uid, uid, None).await { println!("DB Error when trying to unlike: {e}")} }
                _ => { return "Invalid action".into_response(); }
            }
        }
        "project" => {
            match action.as_ref() {
                "like" => { if let Err(e) = db::set_project_like(&state, user.uid, uid, Some(db::Like{is_like:true})).await { println!("DB Error when trying to like: {e}")} }
                "dislike" => { if let Err(e) = db::set_project_like(&state, user.uid, uid, Some(db::Like{is_like:false})).await { println!("DB Error when trying to dislike: {e}")} }
                "unlike" => { if let Err(e) = db::set_project_like(&state, user.uid, uid, None).await { println!("DB Error when trying to unlike: {e}")} }
                _ => { return "Invalid action".into_response(); }
            }
        }
        "user" => {
            match action.as_ref() {
                "like" => { if let Err(e) = db::set_user_like(&state, user.uid, uid, Some(db::Like{is_like:true})).await { println!("DB Error when trying to like: {e}")} }
                "dislike" => { if let Err(e) = db::set_user_like(&state, user.uid, uid, Some(db::Like{is_like:false})).await { println!("DB Error when trying to dislike: {e}")} }
                "unlike" => { if let Err(e) = db::set_user_like(&state, user.uid, uid, None).await { println!("DB Error when trying to unlike: {e}")} }
                _ => { return "Invalid action".into_response(); }
            }
        }
        _ => { return "Invalid type".into_response(); }
    }
    return (StatusCode::OK, [("HX-Trigger", "refreshLikes")], "").into_response();
}

fn _time<F: Fn() -> T, T>(f: F) -> T {
  let start = std::time::SystemTime::now();
  let result = f();
  let end = std::time::SystemTime::now();
  let duration = end.duration_since(start).unwrap();
  println!("it took {} seconds", duration.as_secs());
  result
}