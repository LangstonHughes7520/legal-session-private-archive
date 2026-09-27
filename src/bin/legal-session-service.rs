use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use legal_session_archive::{
    infrai_client::{InfraiClient, InfraiError},
    matter_intake::{open_session, IntakeError, MatterIntake, SessionHandoff},
};
use serde::Serialize;
use std::{env, sync::Arc};

#[derive(Clone)]
struct AppState {
    infrai: InfraiClient,
    bucket: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let infrai = InfraiClient::from_env()?;
    let bucket =
        env::var("LEGAL_ARCHIVE_BUCKET").unwrap_or_else(|_| "legal-session-archive".to_owned());
    if env::args().nth(1).as_deref() == Some("setup-bucket") {
        infrai.create_bucket(&bucket).await?;
        println!("private archive bucket is ready: {bucket}");
        return Ok(());
    }

    let state = Arc::new(AppState { infrai, bucket });
    let app = Router::new()
        .route("/matters/intake", post(intake))
        .with_state(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;
    println!("legal session service listening on http://127.0.0.1:3000");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn intake(
    State(state): State<Arc<AppState>>,
    Json(input): Json<MatterIntake>,
) -> Result<Json<SessionHandoff>, ServiceError> {
    Ok(Json(
        open_session(&state.infrai, &state.bucket, input).await?,
    ))
}

#[derive(Debug)]
struct ServiceError(IntakeError);

impl From<IntakeError> for ServiceError {
    fn from(error: IntakeError) -> Self {
        Self(error)
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ServiceError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            IntakeError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            IntakeError::Infrai(InfraiError::Rejected { status, .. })
                if status.is_client_error() =>
            {
                *status
            }
            IntakeError::Infrai(InfraiError::MissingKey) => StatusCode::INTERNAL_SERVER_ERROR,
            IntakeError::Infrai(_) => StatusCode::BAD_GATEWAY,
        };
        (
            status,
            Json(ErrorBody {
                error: self.0.to_string(),
            }),
        )
            .into_response()
    }
}
