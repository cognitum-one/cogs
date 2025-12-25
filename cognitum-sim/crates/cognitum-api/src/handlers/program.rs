//! Program upload handlers

use actix_web::{web, HttpRequest, HttpResponse};
use chrono::Utc;

use crate::middleware::auth::AuthenticatedUserExt;
use crate::models::{
    error::ApiError,
    request::ProgramMetadata,
    response::ProgramResponse,
    AppState,
};

/// POST /api/v1/programs - Upload program binary
pub async fn create_program(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let user = req.authenticated_user()?;

    // Validate program size
    if body.is_empty() {
        return Err(ApiError::BadRequest("Empty program".to_string()));
    }

    if body.len() > 10 * 1024 * 1024 {
        // 10MB limit
        return Err(ApiError::BadRequest("Program too large".to_string()));
    }

    // Create metadata
    let metadata = ProgramMetadata {
        name: "uploaded_program".to_string(),
        version: "1.0.0".to_string(),
        description: None,
        size: body.len(),
    };

    // Store program
    let program_id = state
        .storage
        .store_program(&body, metadata.clone(), user.user_id)
        .await?;

    let response = ProgramResponse {
        id: program_id,
        size: body.len(),
        metadata,
        uploaded_at: Utc::now(),
    };

    Ok(HttpResponse::Created().json(response))
}

/// GET /api/v1/programs/{id} - Get program
pub async fn get_program(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _user = req.authenticated_user()?;
    let program_id = path.into_inner();

    let (data, metadata) = state.storage.get_program(program_id.clone()).await?;

    let response = ProgramResponse {
        id: program_id,
        size: data.len(),
        metadata,
        uploaded_at: Utc::now(),
    };

    Ok(HttpResponse::Ok().json(response))
}
