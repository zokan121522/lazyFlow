use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use std::sync::Arc;

use crate::error::AppError;
use crate::middleware::AuthenticatedUser;
use crate::models::{Board, CardResponse, CreateCardRequest, UpdateCardRequest};
use crate::AppState;

// ── GET /api/board ───────────────────────────────────────────────────────────

pub async fn get_board(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
) -> Result<Json<Board>, AppError> {
    let board = sqlx::query_as::<_, Board>(
        "SELECT id, owner_id, name, data, updated_at FROM boards WHERE owner_id = $1",
    )
    .bind(user.0)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound("board not found".into()))?;

    Ok(Json(board))
}

// ── PUT /api/board ───────────────────────────────────────────────────────────

pub async fn update_board(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Json(body): Json<Value>,
) -> Result<Json<Board>, AppError> {
    let board = sqlx::query_as::<_, Board>(
        "INSERT INTO boards (owner_id, name, data)
         VALUES ($1, 'default', $2)
         ON CONFLICT (owner_id) DO UPDATE
           SET data = $2, updated_at = NOW()
         RETURNING id, owner_id, name, data, updated_at",
    )
    .bind(user.0)
    .bind(&body)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(board))
}

// ── POST /api/board/cards ────────────────────────────────────────────────────

pub async fn create_card(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Json(req): Json<CreateCardRequest>,
) -> Result<(StatusCode, Json<CardResponse>), AppError> {
    let mut board = sqlx::query_as::<_, Board>(
        "SELECT id, owner_id, name, data, updated_at FROM boards WHERE owner_id = $1",
    )
    .bind(user.0)
    .fetch_one(&state.db)
    .await?;

    let card_id = Uuid::new_v4().to_string();
    let card = json!({
        "id": card_id,
        "column": req.column,
        "title": req.title,
        "body": req.body,
    });

    let cols = board
        .data
        .as_object_mut()
        .ok_or_else(|| AppError::Internal("invalid board data".into()))?;

    let col_cards = cols
        .entry(req.column.clone())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or_else(|| AppError::Internal("column is not an array".into()))?;

    col_cards.push(card);

    sqlx::query("UPDATE boards SET data = $1, updated_at = NOW() WHERE id = $2")
        .bind(&board.data)
        .bind(board.id)
        .execute(&state.db)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(CardResponse {
            id: card_id,
            column: req.column,
            title: req.title,
            body: req.body,
        }),
    ))
}

// ── PUT /api/board/cards/{id} ────────────────────────────────────────────────

pub async fn update_card(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(card_id): Path<String>,
    Json(req): Json<UpdateCardRequest>,
) -> Result<Json<CardResponse>, AppError> {
    let mut board = sqlx::query_as::<_, Board>(
        "SELECT id, owner_id, name, data, updated_at FROM boards WHERE owner_id = $1",
    )
    .bind(user.0)
    .fetch_one(&state.db)
    .await?;

    let cols = board
        .data
        .as_object_mut()
        .ok_or_else(|| AppError::Internal("invalid board data".into()))?;

    let mut found_card: Option<CardResponse> = None;

    for (_col_name, cards) in cols.iter_mut() {
        let arr = cards
            .as_array_mut()
            .ok_or_else(|| AppError::Internal("invalid column data".into()))?;

        for card in arr.iter_mut() {
            if card.get("id").and_then(|v| v.as_str()) == Some(&card_id) {
                if let Some(title) = &req.title {
                    card["title"] = json!(title);
                }
                if let Some(body) = &req.body {
                    card["body"] = json!(body);
                }
                if let Some(column) = &req.column {
                    card["column"] = json!(column);
                }

                found_card = Some(CardResponse {
                    id: card_id.clone(),
                    column: req
                        .column
                        .clone()
                        .or_else(|| card.get("column").and_then(|v| v.as_str().map(String::from)))
                        .unwrap_or_default(),
                    title: req
                        .title
                        .clone()
                        .or_else(|| card.get("title").and_then(|v| v.as_str().map(String::from)))
                        .unwrap_or_default(),
                    body: req
                        .body
                        .clone()
                        .or_else(|| card.get("body").and_then(|v| v.as_str().map(String::from)))
                        .unwrap_or_default(),
                });
                break;
            }
        }
        if found_card.is_some() {
            break;
        }
    }

    let card_response =
        found_card.ok_or_else(|| AppError::NotFound("card not found".into()))?;

    sqlx::query("UPDATE boards SET data = $1, updated_at = NOW() WHERE id = $2")
        .bind(&board.data)
        .bind(board.id)
        .execute(&state.db)
        .await?;

    Ok(Json(card_response))
}

// ── DELETE /api/board/cards/{id} ─────────────────────────────────────────────

pub async fn delete_card(
    State(state): State<Arc<AppState>>,
    user: AuthenticatedUser,
    Path(card_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let mut board = sqlx::query_as::<_, Board>(
        "SELECT id, owner_id, name, data, updated_at FROM boards WHERE owner_id = $1",
    )
    .bind(user.0)
    .fetch_one(&state.db)
    .await?;

    let cols = board
        .data
        .as_object_mut()
        .ok_or_else(|| AppError::Internal("invalid board data".into()))?;

    let mut found = false;
    let col_names: Vec<String> = cols.keys().cloned().collect();

    for col_name in col_names {
        if let Some(cards) = cols.get_mut(&col_name).and_then(|c| c.as_array_mut()) {
            let before = cards.len();
            cards.retain(|c| c.get("id").and_then(|v| v.as_str()) != Some(&card_id));
            if cards.len() < before {
                found = true;
                if cards.is_empty() {
                    cols.remove(&col_name);
                }
                break;
            }
        }
    }

    if !found {
        return Err(AppError::NotFound("card not found".into()));
    }

    sqlx::query("UPDATE boards SET data = $1, updated_at = NOW() WHERE id = $2")
        .bind(&board.data)
        .bind(board.id)
        .execute(&state.db)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

use uuid::Uuid;
