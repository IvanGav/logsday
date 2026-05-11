use askama::Template;
use axum::{
    Form, Router, body::Bytes, http::{HeaderMap, header}, response::{Html, IntoResponse, Redirect}, routing::{get,post}
};
use axum::extract::{Path, Query, State, Multipart};
use axum_typed_multipart::{FieldData, TryFromMultipart, TypedMultipart};
use std::{collections::HashMap, fs};
use tower_http::services::ServeDir;
use sqlx::sqlite::SqlitePool;
use serde::Deserialize;

const MAX_THUMBNAIL_SIZE: u32 = 2 * 1024 * 1024;

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

#[derive(Deserialize)]
struct LoginSubmission {
    username: String,
    password: String,
}

#[derive(Debug, sqlx::FromRow)]
struct User {
    uid: i64, // unique
    displayname: String,
    username: String, // unique
    password: String, // obviously not going to be stored as plaintext password at some point
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

async fn login_handler(
    State(state): State<AppState>,
    Form(payload): Form<LoginSubmission>,
) -> Html<String> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE name = ?")
        .bind(&payload.username)
        .fetch_optional(&state.db)
        .await
        .unwrap();
    match user {
        Some(u) if u.password == payload.password => { Html(format!("<p>Welcome back, {}!</p>", u.displayname)) }
        _ => { Html("<p>Invalid username or password.</p>".to_string()) }
    }
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

// cargo remove axum_typed_multipart if i don't think i need it
async fn post_newproject(data: TypedMultipart<NewProjectRequest>) -> impl IntoResponse {
    println!("{} - {} : {}", data.title, data.description, data.thumbnail.contents.len());
    if let Err(_) = fs::write("DOWNLOADED.jpg", &data.thumbnail.contents) {
        return "Not Ok";
    }
    return "Ok";
}
// async fn post_newproject(mut multipart: Multipart) -> impl IntoResponse {
//     let mut error_message: Option<String> = None;
//     while let Ok(Some(field)) = multipart.next_field().await {
//         let name = field.name().unwrap();
//         match name {
//             "title" => {
//                 let data = field.text().await.unwrap();
//                 println!("Title: {}", data);
//             }
//             "thumbnail" => {
//                 let filename = field.file_name().unwrap().to_string();
//                 let content_type = field.content_type().unwrap_or("unknown");
//                 match content_type {
//                     "image/jpeg" | "image/jpg" | "image/png" => { }
//                     _ => { error_message = Some(format!("Invalid file type: {}", content_type)); }
//                 }
//                 let data = field.bytes().await;
//                 match data {
//                     Ok(data) => {
//                         println!("Filename: {}, Size: {}", filename, data.len());
//                     }
//                     Err(err) => {
//                         println!("Error: {:?}", err);
//                         error_message = Some(format!("thumbnail is too large: max upload size: {}", MAX_THUMBNAIL_SIZE));
//                         return error_message.unwrap().into_response();
//                     }
//                 }
//             }
//             _ => {
//                 println!("Unknown field name: {}", name);
//             }
//         }
//     }
//     if let Some(error_message) = error_message {
//         return error_message.into_response();
//     }
//     return ([("HX-Redirect", "/")], "Redirecting...").into_response();
// }

// async fn expand_update(
//     State(state): State<AppState>,
//     Path(update_id): Path<i64>, // Axum parses this automatically
// ) -> Html<String> {
//     let update = sqlx::query_as::<_, LogEntry>("SELECT * FROM updates WHERE uid = ?")
//         .bind(update_id)
//         .fetch_one(&state.db)
//         .await
//         .unwrap();

//     // Return the expanded fragment
//     Html(format!("<div class='expanded'>{}</div>", update.content))
// }

// async fn get_logs_of_project() -> {
//     let project = sqlx::query_as!(Project, "SELECT * FROM projects WHERE pid = ?", pid)
//         .fetch_one(&state.db)
//         .await?;
//     let logs = sqlx::query_as!(Log, "SELECT * FROM logs WHERE project_id = ? ORDER BY created_at DESC", pid)
//         .fetch_all(&state.db)
//         .await?;
// }