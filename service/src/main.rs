use std::str::FromStr;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use chrono::{Datelike, Local, NaiveDate};
use sea_orm::*;
use sea_orm_migration::prelude::*;
use serde::Deserialize;
use serde_json::json;

use urlencoding::encode;
mod entities;
use entities::{prelude::*, *};

mod migrator;
use migrator::Migrator;

#[derive(Clone)]
struct AppState {
    db: DatabaseConnection,
}

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

    println!("Connecting to database: {}", database_url);
    let db = Database::connect(format!("{}/{}", database_url, database_name))
        .await
        .unwrap();
    Migrator::refresh(&db).await.unwrap();
    let shared_state = AppState { db };
    let app = Router::new()
        .route("/hello/:username", get(get_birthday))
        .route("/hello/:username", put(set_birthday))
        .with_state(shared_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:80").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_birthday(
    Path(name): Path<String>,
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rec: Option<record::Model> = Record::find()
        .filter(record::Column::Username.eq(&name))
        .one(&state.db)
        .await
        .unwrap();
    match rec {
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({ "message": "User not found" })),
        ),
        Some(rec) => {
            let current_date = Local::now().date_naive();
            let this_year_birthday = NaiveDate::from_ymd_opt(
                current_date.year(),
                rec.birthday.month(),
                rec.birthday.day(),
            );
            let current_date = if this_year_birthday.unwrap() < current_date {
                NaiveDate::from_ymd_opt(
                    current_date.year() + 1,
                    rec.birthday.month(),
                    rec.birthday.day(),
                )
                .unwrap()
            } else {
                current_date
            };
            let diff = this_year_birthday.unwrap() - current_date;
            let days = diff.num_days();
            let greeting = format!("Hello, {}! Your birthday is in {} days", name, days);
            (StatusCode::OK, Json(json!({ "message": greeting })))
        }
    }
}

#[derive(Deserialize)]
struct RequestBody {
    dateOfBirth: NaiveDate,
}

async fn set_birthday(
    Path(name): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<RequestBody>,
) -> impl IntoResponse {
    let new_record = record::ActiveModel {
        username: Set(name),
        birthday: Set(NaiveDate::from_str(&payload.dateOfBirth.to_string()).unwrap()),
        ..Default::default()
    };
    match Record::insert(new_record).exec(&state.db).await {
        Ok(_) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
