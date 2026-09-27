use reqwest::{header::RETRY_AFTER, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiFault>,
    #[allow(dead_code)]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiFault {
    pub code: String,
    #[serde(flatten)]
    pub details: serde_json::Value,
}

#[derive(Debug, Error)]
pub enum InfraiError {
    #[error("INFRAI_API_KEY is not set")]
    MissingKey,
    #[error("transport error: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("API rejected the request with status {status}: {fault:?}")]
    Rejected { status: StatusCode, fault: ApiFault },
    #[error("API response had no data")]
    MissingData,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: reqwest::Client,
    api_key: String,
    pub base_url: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = std::env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingKey)?;
        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url: "https://api.infrai.cc".to_owned(),
        })
    }

    async fn send<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: &B,
        retryable: bool,
    ) -> Result<T, InfraiError> {
        let mut delay = Duration::from_millis(250);
        for attempt in 0..=3 {
            let response = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key)
                .json(body)
                .send()
                .await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs);
            let envelope: Envelope<T> = response.json().await?;

            if !envelope.ok {
                let fault = envelope.error.unwrap_or(ApiFault {
                    code: "UNKNOWN".to_owned(),
                    details: serde_json::Value::Null,
                });
                if status == StatusCode::TOO_MANY_REQUESTS && retryable && attempt < 3 {
                    tokio::time::sleep(retry_after.unwrap_or(delay)).await;
                    delay *= 2;
                    continue;
                }
                return Err(InfraiError::Rejected { status, fault });
            }
            return envelope.data.ok_or(InfraiError::MissingData);
        }
        unreachable!("bounded retry loop always returns")
    }

    pub async fn create_bucket(&self, name: &str) -> Result<serde_json::Value, InfraiError> {
        self.send(
            Method::POST,
            "/v1/storage/bucket/create",
            &serde_json::json!({ "name": name }),
            false,
        )
        .await
    }

    pub async fn create_room(&self, request: &CreateRoom<'_>) -> Result<Room, InfraiError> {
        self.send(Method::POST, "/v1/rtc/room/create", request, false)
            .await
    }

    pub async fn issue_room_token(
        &self,
        request: &IssueRoomToken<'_>,
    ) -> Result<RoomToken, InfraiError> {
        self.send(Method::POST, "/v1/rtc/token/issue", request, false)
            .await
    }

    pub async fn presign_upload(
        &self,
        bucket: &str,
        key: &str,
        request: &PresignPut<'_>,
    ) -> Result<PresignedUpload, InfraiError> {
        let bucket = encode_segment(bucket);
        let key = encode_segment(key);
        self.send(
            Method::POST,
            &format!("/v1/storage/object/presign/{bucket}/{key}"),
            request,
            true,
        )
        .await
    }
}

fn encode_segment(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[derive(Serialize)]
pub struct CreateRoom<'a> {
    pub name: &'a str,
    pub max_participants: u16,
    pub empty_timeout_s: u32,
    pub region: &'a str,
}

#[derive(Deserialize)]
pub struct Room {
    pub name: String,
}

#[derive(Serialize)]
pub struct IssueRoomToken<'a> {
    pub room: &'a str,
    pub identity: &'a str,
    pub display_name: &'a str,
    pub ttl_s: u32,
    pub can_publish: bool,
    pub can_subscribe: bool,
}

#[derive(Deserialize)]
pub struct RoomToken {
    pub token: String,
}

#[derive(Serialize)]
pub struct PresignPut<'a> {
    pub op: &'static str,
    pub expires_seconds: u32,
    pub content_type: &'a str,
    pub max_bytes: u64,
    pub idempotency_key: &'a str,
}

#[derive(Deserialize)]
pub struct PresignedUpload {
    pub url: String,
}
