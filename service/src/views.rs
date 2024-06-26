use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{Datelike, Local, NaiveDate};
use sea_orm::*;
use serde::Deserialize;
use serde_json::json;

use crate::entities::{prelude::Record, record};
use std::str::FromStr;

#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
}

pub async fn get_birthday(
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
pub struct RequestBody {
    pub dateOfBirth: NaiveDate,
}

pub async fn set_birthday(
    Path(name): Path<String>,
    State(state): State<AppState>,
    Json(payload): Json<RequestBody>,
) -> impl IntoResponse {
    if !(name.chars().all(char::is_alphabetic)) {
        return StatusCode::BAD_REQUEST;
    }
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
