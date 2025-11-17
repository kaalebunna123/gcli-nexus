use axum::{
    Json,
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use oauth2::basic::BasicErrorResponseType;
use oauth2::reqwest::Error as ReqwestClientError;
use oauth2::{HttpClientError, RequestTokenError, StandardErrorResponse};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Error as SqlxError;
use std::collections::HashMap;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum NexusError {
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("HTTP request error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Missing access token; refresh first")]
    MissingAccessToken,

    #[error("missing email in userinfo response")]
    MissingEmailInUserinfo,

    #[error("OAuth2 token request error: {0}")]
    Oauth2Token(String),

    #[error("OAuth2 server error: {error}")]
    Oauth2Server { error: String },

    #[error("No available credential")]
    NoAvailableCredential,

    #[error("Ractor error: {0}")]
    RactorError(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] SqlxError),

    #[error("upstream HTTP error: {status}")]
    UpstreamHttp {
        status: StatusCode,
        headers: HeaderMap,
        body: Bytes,
    },
    #[error("Gemini API error: {0:?}")]
    GeminiServerError(GeminiError),
}

impl NexusError {}

impl
    From<
        RequestTokenError<
            HttpClientError<ReqwestClientError>,
            StandardErrorResponse<BasicErrorResponseType>,
        >,
    > for NexusError
{
    fn from(
        e: RequestTokenError<
            HttpClientError<ReqwestClientError>,
            StandardErrorResponse<BasicErrorResponseType>,
        >,
    ) -> Self {
        match e {
            RequestTokenError::ServerResponse(err) => NexusError::Oauth2Server {
                error: err.error().to_string(),
            },
            RequestTokenError::Request(req_e) => {
                NexusError::Oauth2Token(format!("request failed: {}", req_e))
            }
            RequestTokenError::Parse(parse_err, _body) => NexusError::Json(parse_err.into_inner()),
            RequestTokenError::Other(s) => NexusError::Oauth2Token(s),
        }
    }
}
impl IntoResponse for NexusError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_body) = match self {
            NexusError::GeminiServerError(gemini_err) => {
                let status = StatusCode::from_u16(gemini_err.error.code as u16)
                    .unwrap_or(StatusCode::BAD_REQUEST);

                let body = ApiErrorBody {
                    code: gemini_err.error.status,
                    message: gemini_err.error.message,
                };
                (status, body)
            }
            NexusError::DatabaseError(_) | NexusError::RactorError(_) => {
                let status = StatusCode::INTERNAL_SERVER_ERROR;
                let body = ApiErrorBody {
                    code: "INTERNAL_ERROR".to_string(),
                    message: "An internal server error occurred.".to_string(),
                };
                (status, body)
            }
            NexusError::Json(_)
            | NexusError::Oauth2Token(_)
            | NexusError::Oauth2Server { .. }
            | NexusError::MissingAccessToken
            | NexusError::MissingEmailInUserinfo => {
                let status = StatusCode::UNAUTHORIZED;
                let body = ApiErrorBody {
                    code: "UNAUTHORIZED".to_string(),
                    message: "Authentication error.".to_string(),
                };
                (status, body)
            }
            NexusError::NoAvailableCredential => {
                let status = StatusCode::SERVICE_UNAVAILABLE; // 503
                let body = ApiErrorBody {
                    code: "NO_CREDENTIAL".to_string(),
                    message: "No available credentials to process the request.".to_string(),
                };
                (status, body)
            }
            NexusError::Reqwest(_) | NexusError::UrlParse(_) => {
                let status = StatusCode::BAD_GATEWAY;
                let body = ApiErrorBody {
                    code: "BAD_GATEWAY".to_string(),
                    message: "Upstream service is unavailable.".to_string(),
                };
                (status, body)
            }
            NexusError::UpstreamHttp {
                status,
                headers,
                body,
            } => {
                let status = status;
                let body = ApiErrorBody {
                    code: "BAD_GATEWAY".to_string(),
                    message: "Upstream service is unavailable.".to_string(),
                };
                (status, body)
            }
        };
        (status, Json(ApiErrorResponse { error: error_body })).into_response()
    }
}

/// Standardized API error response body
#[derive(Serialize)]
pub struct ApiErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct ApiErrorResponse {
    pub error: ApiErrorBody,
}

/// Gemini API error response structure
#[derive(Deserialize, Debug)]
pub struct GeminiError {
    pub error: GeminiErrorBody,
}

#[derive(Deserialize, Debug)]
pub struct GeminiErrorBody {
    pub code: u32,
    pub message: String,
    pub status: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}
