use actix_web::{get, HttpResponse, Result as ActixResult};

/// Health check endpoint
#[get("/health")]
pub async fn health() -> ActixResult<HttpResponse> {
    Ok(HttpResponse::Ok().json(common::HealthResponse {
        status: "ok".to_string(),
    }))
}

