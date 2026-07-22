use axum::{
    extract::{Json, Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension,
};
use calamine::{Reader, Xlsx, open_workbook_from_rs};
use chrono::Utc;
use mongodb::{
    bson::{doc, oid::ObjectId},
    Collection,
};
use serde::Serialize;
use std::io::Cursor;
use std::sync::Arc;

use crate::{
    error::{AppError, Result},
    middleware::AuthenticatedAdmin,
    models::{Batch, BatchCreate, Student},
};

#[derive(Debug, Serialize)]
pub struct BatchResponse {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "studentCount")]
    pub student_count: usize,
}

pub async fn create_batch(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    Json(payload): Json<BatchCreate>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Batch> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Batch::collection_name());

    let batch = Batch {
        id: None,
        name: payload.name,
        description: payload.description,
        students: payload.students,
        created_by: auth.id,
        created_at: Utc::now(),
    };

    let result = collection.insert_one(&batch).await?;
    let batch_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(BatchResponse {
            id: batch_id.to_hex(),
            name: batch.name,
            description: batch.description,
            student_count: batch.students.len(),
        }),
    ))
}

pub async fn get_batches(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Batch> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Batch::collection_name());

    let mut cursor = collection
        .find(doc! {})
        .sort(doc! { "createdAt": -1 })
        .await?;
    let mut batches = Vec::new();

    while cursor.advance().await? {
        let batch = cursor.deserialize_current()?;
        batches.push(BatchResponse {
            id: batch
                .id
                .ok_or_else(|| AppError::Internal("No ID".to_string()))?
                .to_hex(),
            name: batch.name,
            description: batch.description,
            student_count: batch.students.len(),
        });
    }

    Ok(Json(batches))
}

pub async fn get_batch(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Batch> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Batch::collection_name());

    let batch_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;

    let batch = collection
        .find_one(doc! { "_id": batch_id })
        .await?
        .ok_or_else(|| AppError::NotFound("Batch not found".to_string()))?;

    Ok(Json(BatchResponse {
        id: batch.id.unwrap().to_hex(),
        name: batch.name,
        description: batch.description,
        student_count: batch.students.len(),
    }))
}

pub async fn delete_batch(
    State(state): State<Arc<crate::AppState>>,
    Extension(_auth): Extension<AuthenticatedAdmin>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse> {
    let collection: Collection<Batch> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Batch::collection_name());

    let batch_id = ObjectId::parse_str(&id)
        .map_err(|e| AppError::BadRequest(format!("Invalid batch ID: {}", e)))?;

    collection.delete_one(doc! { "_id": batch_id }).await?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize)]
pub struct UploadBatchResponse {
    pub batch_id: String,
    pub name: String,
    pub students_imported: usize,
    pub students_skipped: usize,
    pub errors: Vec<String>,
}

pub async fn upload_batch_excel(
    State(state): State<Arc<crate::AppState>>,
    Extension(auth): Extension<AuthenticatedAdmin>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut batch_name: Option<String> = None;
    let mut description: Option<String> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        AppError::BadRequest(format!("Multipart error: {}", e))
    })? {
        let name = field.name().unwrap_or("").to_string();
        
        match name.as_str() {
            "file" => {
                file_data = Some(field.bytes().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read file: {}", e))
                })?.to_vec());
            }
            "name" => {
                batch_name = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read name: {}", e))
                })?);
            }
            "description" => {
                description = Some(field.text().await.map_err(|e| {
                    AppError::BadRequest(format!("Failed to read description: {}", e))
                })?);
            }
            _ => {}
        }
    }

    let file_data = file_data.ok_or_else(|| {
        AppError::BadRequest("No file uploaded".to_string())
    })?;

    let batch_name = batch_name.unwrap_or_else(|| {
        format!("Batch_{}", Utc::now().format("%Y%m%d_%H%M%S"))
    });

    let (students, errors) = parse_excel(&file_data)?;

    let collection: Collection<Batch> = state
        .db
        .database(
            state
                .config
                .mongodb_uri
                .split('/')
                .next_back()
                .unwrap_or("default"),
        )
        .collection(Batch::collection_name());

    let batch = Batch {
        id: None,
        name: batch_name.clone(),
        description,
        students: students.clone(),
        created_by: auth.id,
        created_at: Utc::now(),
    };

    let result = collection.insert_one(&batch).await?;
    let batch_id = result
        .inserted_id
        .as_object_id()
        .ok_or_else(|| AppError::Internal("Failed to get inserted ID".to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(UploadBatchResponse {
            batch_id: batch_id.to_hex(),
            name: batch_name,
            students_imported: students.len(),
            students_skipped: 0,
            errors,
        }),
    ))
}

fn parse_excel(data: &[u8]) -> Result<(Vec<Student>, Vec<String>)> {
    let cursor = Cursor::new(data);
    let mut workbook: Xlsx<_> = open_workbook_from_rs(cursor)
        .map_err(|e| AppError::BadRequest(format!("Failed to open Excel file: {}", e)))?;

    let range = workbook.worksheet_range_at(0)
        .ok_or_else(|| AppError::BadRequest("No worksheet found".to_string()))?
        .map_err(|e| AppError::BadRequest(format!("Failed to read worksheet: {}", e)))?;

    let mut students = Vec::new();
    let mut errors = Vec::new();

    let rows: Vec<_> = range.rows().collect();
    
    if rows.is_empty() {
        return Ok((students, errors));
    }

    let header_row = rows[0];
    let mut col_map = std::collections::HashMap::new();
    
    for (i, cell) in header_row.iter().enumerate() {
        let header_str = match cell {
            calamine::Data::String(s) => s.clone(),
            _ => continue,
        };
        let header_lower = header_str.to_lowercase().trim().to_string();
        col_map.insert(header_lower, i);
    }

    let name_col = col_map.get("name").or_else(|| col_map.get("student name")).or_else(|| col_map.get("student_name"));
    let roll_col = col_map.get("roll").or_else(|| col_map.get("roll number")).or_else(|| col_map.get("roll_number"));
    let email_col = col_map.get("email").or_else(|| col_map.get("email id"));
    let college_col = col_map.get("college").or_else(|| col_map.get("college name"));

    for (row_idx, row) in rows.iter().skip(1).enumerate() {
        let get_string_val = |col: Option<&usize>, row: &[calamine::Data]| -> Option<String> {
            col.and_then(|&i| {
                row.get(i).and_then(|cell| {
                    match cell {
                        calamine::Data::String(s) => Some(s.clone()),
                        calamine::Data::Int(n) => Some(n.to_string()),
                        calamine::Data::Float(f) => Some(f.to_string()),
                        _ => None,
                    }
                })
            })
        };

        let name = get_string_val(name_col, row);
        let roll_number = get_string_val(roll_col, row);
        let email = get_string_val(email_col, row);
        let college_name = get_string_val(college_col, row);

        match (&name, &roll_number) {
            (Some(n), Some(r)) if !n.trim().is_empty() && !r.trim().is_empty() => {
                students.push(Student {
                    name: n.trim().to_string(),
                    roll_number: r.trim().to_string(),
                    email: email.map(|e| e.trim().to_string()),
                    college_name: college_name.map(|c| c.trim().to_string()),
                });
            }
            _ => {
                if name.is_some() || roll_number.is_some() {
                    errors.push(format!("Row {}: Missing required fields (name/roll_number)", row_idx + 2));
                }
            }
        }
    }

    Ok((students, errors))
}
