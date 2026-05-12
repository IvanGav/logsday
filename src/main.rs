use askama::Template;
use axum::{
    Form, Router, body::Bytes, http::{HeaderMap, StatusCode, header}, response::{Html, IntoResponse, Redirect}, routing::{get,post}
};
use axum::extract::{Path, Query, State, Multipart};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use tokio::net::unix::uid_t;
use std::{collections::HashMap, fs};
use tower_http::services::ServeDir;
use sqlx::sqlite::SqlitePool;
use serde::Deserialize;
use tower_sessions::{Expiry, MemoryStore, SessionManagerLayer, cookie::time};
use tower_sessions::Session;

mod db;

// cargo remove axum_typed_multipart if i don't think i need it

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
        .with_secure(false) // set to true later when you have HTTPS
        .with_expiry(Expiry::OnInactivity(time::Duration::days(1)));

    let state = AppState { db: db_pool };

    let app = Router::new()
        .route("/", get(landing))
        .route("/signup", get(get_signup).post(post_signup))
        .route("/login", get(get_login).post(post_login))
        .route("/my", get(get_dashboard))
        .route("/my/projects", get(get_my_projects))
        .route("/newproject", get(get_newproject).post(post_newproject))
        .route("/conman", get(get_conman))
        .route("/favicon.ico", get(get_favicon))
        .nest_service("/static", ServeDir::new("public"))
        .layer(session_layer)
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn generic_error_html() -> Html<String> {
    return Html("Oops, something went wrong... Go touch some logs in the meantime.".to_string());
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
}

#[derive(Debug, sqlx::FromRow)]
struct Project {
    uid: i64, // unique
    user_uid: i64,
    title: String,
    slug: String,
    description: String, // nullable
    thumbnail_path: String,
    // created_on: DateTime,
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
    // created_on: DateTime,
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
#[template(path = "page/landing.html")]
struct LandingTemplate;

#[derive(Template)]
#[template(path = "prototype/base.html")]
struct PrototypeTemplate;

async fn landing() -> Html<String> {
    let render = LandingTemplate.render(); //PrototypeTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return generic_error_html();
}

// Route /conman

#[derive(Template)]
#[template(path = "page/conman.html")]
struct ConmanTemplate;

async fn get_conman() -> Html<String> {
    let render = ConmanTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return generic_error_html();
}

// Route /favicon.ico

async fn get_favicon() -> impl IntoResponse {
    // Bake the file into the binary at compile time - pretty cool. Did you know I like Rust?
    let bytes = include_bytes!("../public/favicon.ico");
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/x-icon".parse().unwrap());
    (headers, bytes)
}

// Route /newproject

#[derive(Template)]
#[template(path = "page/newproject.html")]
struct NewProjectTemplate;

async fn get_newproject(session: Session) -> impl IntoResponse {
    let uid: Option<i64> = session.get("uid").await.unwrap();
    if let None = uid { return Redirect::to("/login").into_response(); } //hx_redirect("/login".into()).into_response(); }
    let render = NewProjectTemplate.render();
    if let Ok(render) = render {
        return Html(render).into_response();
    }
    return generic_error_html().into_response();
}

#[derive(TryFromMultipart)]
struct NewProjectRequest {
    #[form_data(field_name = "title")]
    title: String,
    #[form_data(field_name = "description")]
    description: String,
    #[form_data(field_name = "thumbnail", limit = "2MB")]
    thumbnail: FieldData<Bytes>,
}

async fn post_newproject(State(state): State<AppState>, session: Session, data: TypedMultipart<NewProjectRequest>) -> impl IntoResponse {
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
    let proj_slug = data.title.to_lowercase();
    let project_path = format!("storage/users/{}/{}", &u.username, &data.title);
    let thumbnail_path = format!("{}/{}", &project_path, "thumb.jpg");
    if let Ok(_) = db::create_project(&state, uid, &data.title, &proj_slug, &data.description, &thumbnail_path).await {
        if let Ok(_) = tokio::fs::create_dir_all(project_path).await {
            if let Ok(_) = fs::write(thumbnail_path, &data.thumbnail.contents) {
                return hx_redirect("/my".into()).into_response();
            }
        }
    }
    return generic_error_html().into_response();
}

// Route /signup
#[derive(Template)]
#[template(path = "page/signup.html")]
struct SignupTemplate;

async fn get_signup() -> Html<String> {
    let render = SignupTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return generic_error_html();
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
    // TODO make sure that username/displayname have valid structure
    let result = sqlx::query(
        "INSERT INTO users (username, displayname, password) VALUES (?, ?, ?)",
    )
        .bind(&form.username)
        .bind(&form.displayname)
        .bind(&form.password) // plaintext password go brrrr
        .execute(&state.db)
        .await;

    match result {
        Ok(_) => {
            if let Ok(_) = tokio::fs::create_dir_all(format!("storage/users/{}", form.username)).await {
                let u = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?;")
                    .bind(&form.username)
                    .fetch_one(&state.db)
                    .await;
                if let Err(_) = u { return (StatusCode::INTERNAL_SERVER_ERROR, "Couldn't find user after creating").into_response(); }
                let uid = u.unwrap().uid;
                session.insert("uid", uid).await.unwrap();
                return hx_redirect("/my".into()).into_response();
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
#[template(path = "page/login.html")]
struct LoginTemplate;

async fn get_login() -> Html<String> {
    let render = LoginTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return generic_error_html();
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
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&form.username)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    match user {
        Some(u) if u.password == form.password => {
            session.insert("uid", u.uid).await.unwrap();
            return hx_redirect("/my".into()).into_response();
        }
        _ => { return "Incorrect Username or Password".into_response(); }
    };
}

// Route /my
#[derive(Template)]
#[template(path = "page/dashboard.html")]
struct DashboardTemplate;

async fn get_dashboard(session: Session) -> impl IntoResponse {
    let user_id: Option<i64> = session.get("uid").await.unwrap();
    match user_id {
        Some(_) => {
            let render = DashboardTemplate.render();
            if let Ok(render) = render {
                return Html(render).into_response();
            }
            return generic_error_html().into_response();
        },
        None => {
            // hx_redirect("/login".into()).into_response()
            return Redirect::to("/login").into_response();
        },
    };
}

// Route /my/projects
#[derive(Template)]
#[template(path = "bits/project_list.html")]
struct ProjectListTemplate {
    projects: Vec<Project>
}

async fn get_my_projects(session: Session, State(state): State<AppState>) -> impl IntoResponse {
    let user_id: Option<i64> = session.get("uid").await.unwrap();
    match user_id {
        Some(uid) => {
            let projects = sqlx::query_as::<_,Project>("SELECT * FROM projects WHERE user_uid = ?;")
                .bind(&uid)
                .fetch_all(&state.db)
                .await;
            if let Err(e) = projects { return generic_error_html().into_response(); }
            let projects = projects.unwrap();
            let render = ProjectListTemplate{projects}.render();
            if let Ok(render) = render {
                return Html(render).into_response();
            }
            return generic_error_html().into_response();
        },
        // None => hx_redirect("/login".into()).into_response(),
        None => Redirect::to("/login").into_response()
    }
}