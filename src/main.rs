use askama::Template;
use axum::{
    Form, Router, body::Bytes, http::{HeaderMap, StatusCode, header}, response::{Html, IntoResponse, Redirect}, routing::{get,post}
};
use axum::extract::{Path, Query, State, Multipart};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use std::{collections::HashMap, fs};
use tower_http::services::ServeDir;
use sqlx::sqlite::SqlitePool;
use serde::Deserialize;

const MAX_THUMBNAIL_SIZE: u32 = 2 * 1024 * 1024;
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

    let state = AppState { db: db_pool };

    let app = Router::new()
        .route("/", get(landing))
        .route("/signup", get(get_signup).post(post_signup))
        .route("/login", get(get_login).post(post_login))
        .route("/newproject", get(get_newproject).post(post_newproject))
        .route("/conman", get(get_conman))
        .route("/favicon.ico", get(get_favicon))
        .nest_service("/static", ServeDir::new("public"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn generic_error_html() -> Html<String> {
    return Html("Oops, something went wrong... Go touch some logs in the meantime.".to_string());
}

fn hx_redirect(route: String, body: String) -> impl IntoResponse {
    return ([("HX-Redirect", route)], body).into_response();
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
    let render = PrototypeTemplate.render(); //LandingTemplate.render();
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

async fn get_newproject() -> Html<String> {
    let render = NewProjectTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return generic_error_html();
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

async fn post_newproject(data: TypedMultipart<NewProjectRequest>) -> impl IntoResponse {
    let content_type = &data.thumbnail.metadata.content_type;
    if let None = content_type { return "Could not get file type".into_response(); }
    let content_type = content_type.as_ref().unwrap();
    if content_type != "image/jpg" && content_type != "image/jpeg" && content_type != "image/png" { return "Unsupported file format".into_response(); }

    println!("-- debug `{}` : `{}` : `{:?}`", data.title, data.description, data.thumbnail.metadata.file_name);
    if let Err(e) = fs::write("DOWNLOADED.jpg", &data.thumbnail.contents) {
        return e.to_string().into_response(); // don't actually do this, of course
    }

    return ([("HX-Redirect", "/")], "Redirecting...").into_response(); // HTMX's own force redirect
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
    State(state): State<AppState>,
    Form(form): Form<SignupSubmission>,
) -> impl IntoResponse {
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
                return hx_redirect("/".into(), "Account created.".into()).into_response();
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
    State(state): State<AppState>,
    Form(form): Form<LoginSubmission>,
) -> impl IntoResponse {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?")
        .bind(&form.username)
        .fetch_optional(&state.db)
        .await
        .unwrap();

    match user {
        Some(u) if u.password == form.password => { return hx_redirect("/".into(), "Success".into()).into_response(); }
        _ => { return "Incorrect Username of Password".into_response(); }
    };
}