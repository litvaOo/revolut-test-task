use axum::{
    routing::{get, put},
    Router,
};
use sea_orm::*;
use sea_orm_migration::prelude::*;
use tower_http::trace::{self, TraceLayer};
use tracing::Level;

use urlencoding::encode;

mod entities;
mod migrator;
use migrator::Migrator;
mod views;
use views::{get_birthday, set_birthday, AppState};

#[tokio::main]
async fn main() {
    let database_host = std::env::var("DATABASE_HOST").unwrap();
    let database_port = std::env::var("DATABASE_PORT").unwrap();
    let database_user = std::env::var("DATABASE_USER").unwrap();
    let database_password = std::env::var("DATABASE_PASSWORD").unwrap();
    let database_url = format!(
        "postgres://{}:{}@{}:{}",
        database_user,
        encode(database_password.as_str()),
        database_host,
        database_port
    );
    let database_name = std::env::var("DATABASE_NAME").unwrap();

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .compact()
        .init();
    let db = Database::connect(format!("{}/{}", database_url, database_name))
        .await
        .unwrap();

    Migrator::refresh(&db).await.unwrap();

    let shared_state = AppState { db };
    let app = Router::new()
        .route("/hello/:username", get(get_birthday))
        .route("/hello/:username", put(set_birthday))
        .with_state(shared_state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        );
    let listener = tokio::net::TcpListener::bind("0.0.0.0:80").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
