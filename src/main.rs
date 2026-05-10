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

struct Log {
    title: String,
    content: String,
}

// apparently need to set env var `DATABASE_URL=sqlite:sqlite.db`
// if the above doesn't work, maybe try `DATABASE_URL="~/logsday/sqlite.db"`

#[tokio::main]
async fn main() {
    let db_pool = SqlitePool::connect("sqlite:sqlite.db")
        .await
        .expect("Could not connect to database");

    let state = AppState { db: db_pool };

    let app = Router::new()
        .route("/", get(landing))
        .route("/conman", get(conman))
        .route("/{user_uid}/get", get(testing))
        .route("/logs/loglist", get(bit_loglist))
        .route("/favicon.ico", get(favicon))
        .route("/users", get(list_users))
        .route("/login", get(login_form).post(login_handler))
        .nest_service("/static", ServeDir::new("public"))
        .with_state(state);

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
}

#[derive(Deserialize)]
struct LoginSubmission {
    username: String,
    password: String, // Reminder: Hash these in a real app!
}

#[derive(Debug, sqlx::FromRow)]
struct User {
    uid: i64,
    name: String,
    password: String,
}

async fn list_users(State(state): State<AppState>) -> String {
    // sqlx::query_as!(...) can be used for compile time checks on queries.. but ...
    let users: Vec<User> = sqlx::query_as("SELECT uid, name, password FROM users")
        .fetch_all(&state.db)
        .await
        .unwrap();

    format!("Found {} users. First user: {:?}", users.len(), users.get(0))
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
        Some(u) if u.password == payload.password => {
            // Success: Return a welcome fragment
            Html(format!("<p>Welcome back, {}!</p>", u.name))
        }
        _ => {
            // Failure: Return the form again with an error message
            // In a real app, you'd render an Askama template here
            Html("<p>Invalid username or password.</p>".to_string())
        }
    }
}

// Query:
// http://localhost:3000/12345/get?test1=hello,test2=world
// params = test1:hello,test2=world,
// Path:
// https://localhost:3000/12345/get
// user_id = 12345
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
#[template(path = "page/landing.html")]
struct LandingTemplate;

#[derive(Template)]
#[template(path = "page/conman.html")]
struct ConmanTemplate;

#[derive(Template)]
#[template(path = "bits/loglist.html")]
struct BitLoglistTemplate {
    logs: Vec<Log>,
}

#[derive(Template)]
#[template(path = "bits/login_form.html")]
struct BitLoginTemplate;

async fn landing() -> Html<String> {
    let render = LandingTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return Html("Something went wrong".to_string());
}

async fn conman() -> Html<String> {
    let render = ConmanTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return Html("Something went wrong".to_string());
}

async fn bit_loglist() -> Html<String> {
    let render = BitLoglistTemplate{logs: vec![Log{title: "My first log".into(), content: "I'm making logsday web".into()},Log{title: "My second log".into(), content: "Struggling".into()}]}.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return Html("Something went wrong".to_string());
}

async fn login_form() -> Html<String> {
    let render = BitLoginTemplate.render();
    if let Ok(render) = render {
        return Html(render);
    }
    return Html("Something went wrong".to_string());
}

async fn favicon() -> impl IntoResponse {
    println!("favicon requested");
    // Bake the file into the binary at compile time - pretty cool. Did you know I like Rust?
    let bytes = include_bytes!("../public/favicon.ico");
    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, "image/x-icon".parse().unwrap());
    (headers, bytes)
}


/*
CREATE TABLE users (
uid int PRIMARY KEY,
name varchar(64) NOT NULL,
password varchar(64) NOT NULL
);
*/