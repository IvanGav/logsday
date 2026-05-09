use axum::{
    routing::get,
    Router,
};

use axum::extract::{Path, Query, Json};
use std::collections::HashMap;
use serde_json::{Value, json};

// Set-Cookie http header

#[tokio::main]
async fn main() {
    // our router
    // let app = Router::new()
    //     .route("/", get(root))
    //     .route("/foo", get(get_foo).post(post_foo))
    //     .route("/foo/bar", get(foo_bar));

    // // which calls one of these handlers
    // async fn root() {}
    // async fn get_foo() {}
    // async fn post_foo() {}
    // async fn foo_bar() {}

    
    // // `Path` gives you the path parameters and deserializes them.
    // async fn path(Path(user_id): Path<u32>) {}

    // // `Query` gives you the query parameters and deserializes them.
    // async fn query(Query(params): Query<HashMap<String, String>>) {}

    // // Buffer the request body and deserialize it as JSON into a
    // // `serde_json::Value`. `Json` supports any type that implements
    // // `serde::Deserialize`.
    // async fn json(Json(payload): Json<serde_json::Value>) {}

    // `&'static str` becomes a `200 OK` with `content-type: text/plain; charset=utf-8`
    async fn plain_text() -> &'static str {
        "hello world"
    }

    // `Json` gives a content-type of `application/json` and works with any type
    // that implements `serde::Serialize`
    async fn json() -> Json<Value> {
        Json(json!({ "data": 42 }))
    }

    let app = Router::new()
        .route("/plain_text", get(plain_text))
        .route("/json", get(json));

    // let app = Router::new().route("/", get(|| async { "Hello, World!" }));

    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}