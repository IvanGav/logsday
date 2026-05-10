use askama::Template;
use axum::{
    routing::{get,post},
    Router,
    response::{Html, IntoResponse},
    http::{header, HeaderMap},
    Form,
};
use axum::extract::{Path, Query, State};
use std::collections::HashMap;
use tower_http::services::ServeDir;
use sqlx::sqlite::SqlitePool;
use serde::Deserialize;

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
        .route("/conman", get(conman))
        .route("/loglist", get(bit_loglist))
        .route("/login", get(login_form).post(login_handler))
        .route("/favicon.ico", get(favicon))
        .nest_service("/static", ServeDir::new("public"))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Deserialize)]
struct LoginSubmission {
    username: String,
    password: String,
}

// TODO remove later, this is just for testing
struct Log {
    title: String,
    content: String,
}

#[derive(Debug, sqlx::FromRow)]
struct User {
    uid: i64,
    name: String,
    password: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LogEntry {
    uid: i64,
    project_uid: i64,
    log_type: bool, // if true, it's a log; if false, it's an update; it's a stupid system, I know, but now i don't have to deal with sqlx enums
    title: String,
    content: String,
}

#[derive(Debug, sqlx::FromRow)]
struct LogHeader {
    uid: i64,
    project_uid: i64,
    log_type: bool, // if true, it's a log; if false, it's an update; it's a stupid system, I know, but now i don't have to deal with sqlx enums
    title: String,
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
        Some(u) if u.password == payload.password => { Html(format!("<p>Welcome back, {}!</p>", u.name)) }
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

// Page Templates

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
    return Html("Something went wrong".to_string());
}

#[derive(Template)]
#[template(path = "page/conman.html")]
struct ConmanTemplate;

async fn conman() -> Html<String> {
    let render = ConmanTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return Html("Something went wrong".to_string());
}

// Bits Templates

#[derive(Template)]
#[template(path = "bits/loglist.html")]
struct BitLoglistTemplate {
    logs: Vec<Log>,
}

async fn bit_loglist() -> Html<String> {
    let render = BitLoglistTemplate{logs: vec![Log{title: "My first log".into(), content: "I'm making logsday web".into()},Log{title: "My second log".into(), content: "Struggling".into()}]}.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return Html("Something went wrong".to_string());
}

#[derive(Template)]
#[template(path = "bits/login_form.html")]
struct BitLoginTemplate;

async fn login_form() -> Html<String> {
    let render = BitLoginTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return Html("Something went wrong".to_string());
}

// Other Handlers, Without Templates

async fn favicon() -> impl IntoResponse {
    // Bake the file into the binary at compile time - pretty cool. Did you know I like Rust?
    let bytes = include_bytes!("../public/favicon.ico");
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/x-icon".parse().unwrap());
    (headers, bytes)
}

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