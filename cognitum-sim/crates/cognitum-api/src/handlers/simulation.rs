//! Simulation handlers

use actix_web::{web, HttpRequest, HttpResponse};
use chrono::Utc;
use validator::Validate;

use crate::middleware::auth::AuthenticatedUserExt;
use crate::models::{
    error::ApiError,
    request::{CreateSimulationRequest, Pagination, RunRequest},
    response::{CreateSimulationResponse, RunResponse, SimulationResults, SimulationStatusResponse},
    AppState,
};

/// POST /api/v1/simulations - Create new simulation
pub async fn create_simulation(
    req: HttpRequest,
    body: web::Json<CreateSimulationRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let user = req.authenticated_user()?;

    // Validate request
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Create simulation
    let sim_id = state
        .simulator
        .create_simulation(body.config.clone(), body.program_id.clone(), user.user_id)
        .await?;

    let response = CreateSimulationResponse {
        id: sim_id,
        status: "created".to_string(),
        config: body.config.clone(),
        created_at: Utc::now(),
    };

    Ok(HttpResponse::Created().json(response))
}

/// GET /api/v1/simulations - List simulations
pub async fn list_simulations(
    req: HttpRequest,
    query: web::Query<Pagination>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let user = req.authenticated_user()?;

    let simulations = state
        .simulator
        .list_simulations(user.user_id, query.into_inner())
        .await?;

    Ok(HttpResponse::Ok().json(simulations))
}

/// GET /api/v1/simulations/{id} - Get simulation details
pub async fn get_simulation(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _user = req.authenticated_user()?;
    let sim_id = path.into_inner();

    let simulation = state.simulator.get_simulation(sim_id).await?;

    Ok(HttpResponse::Ok().json(simulation))
}

/// POST /api/v1/simulations/{id}/run - Start simulation
pub async fn run_simulation(
    req: HttpRequest,
    path: web::Path<String>,
    body: web::Json<RunRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _user = req.authenticated_user()?;
    let sim_id = path.into_inner();

    // Validate request
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let job_id = state
        .simulator
        .run_simulation(sim_id, body.into_inner())
        .await?;

    let response = RunResponse {
        job_id,
        status: "queued".to_string(),
        estimated_completion: Some(Utc::now() + chrono::Duration::seconds(5)),
    };

    Ok(HttpResponse::Accepted().json(response))
}

/// GET /api/v1/simulations/{id}/status - Get simulation status
pub async fn get_status(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _user = req.authenticated_user()?;
    let sim_id = path.into_inner();

    let status = state.simulator.get_status(sim_id).await?;

    Ok(HttpResponse::Ok().json(status))
}

/// GET /api/v1/simulations/{id}/results - Get simulation results
pub async fn get_results(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _user = req.authenticated_user()?;
    let sim_id = path.into_inner();

    let results = state.simulator.get_results(sim_id.clone()).await?;

    let response = SimulationResults {
        simulation_id: sim_id,
        status: "completed".to_string(),
        results,
        completed_at: Some(Utc::now()),
    };

    Ok(HttpResponse::Ok().json(response))
}

/// DELETE /api/v1/simulations/{id} - Delete simulation
pub async fn delete_simulation(
    req: HttpRequest,
    path: web::Path<String>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, ApiError> {
    let _user = req.authenticated_user()?;
    let sim_id = path.into_inner();

    state.simulator.delete_simulation(sim_id).await?;

    Ok(HttpResponse::NoContent().finish())
}
