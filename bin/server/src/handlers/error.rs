use tracing::error;

pub fn handle_error<E: std::fmt::Display>(msg: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorBadRequest(format!("{}: {}", msg, e))
}

pub fn handle_auth_error<E: std::fmt::Display>(msg: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorUnauthorized(format!("{}: {}", msg, e))
}

pub fn handle_server_error<E: std::fmt::Display>(msg: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorInternalServerError(format!("{}: {}", msg, e))
}

pub fn handle_not_found<E: std::fmt::Display>(msg: &str, id: &str, e: E) -> actix_web::Error {
    error!("{}: {}", msg, e);
    actix_web::error::ErrorNotFound(format!("Batch {} not found: {}", id, e))
}
