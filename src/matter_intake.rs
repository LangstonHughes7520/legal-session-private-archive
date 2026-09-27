use crate::infrai_client::{CreateRoom, InfraiClient, InfraiError, IssueRoomToken, PresignPut};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize)]
pub struct MatterIntake {
    pub matter_id: String,
    pub participant_id: String,
    pub participant_name: String,
    pub signed_document_delivered: bool,
}

#[derive(Debug, Serialize)]
pub struct SessionHandoff {
    pub matter_id: String,
    pub room: String,
    pub room_token: String,
    pub recording_upload_url: String,
    pub recording_object_key: String,
    pub follow_up_in_days: u8,
}

#[derive(Debug, Error)]
pub enum IntakeError {
    #[error("{0}")]
    InvalidInput(&'static str),
    #[error(transparent)]
    Infrai(#[from] InfraiError),
}

pub fn follow_up_days(signed_document_delivered: bool) -> u8 {
    if signed_document_delivered {
        7
    } else {
        2
    }
}

pub async fn open_session(
    infrai: &InfraiClient,
    bucket: &str,
    intake: MatterIntake,
) -> Result<SessionHandoff, IntakeError> {
    validate_identifier(&intake.matter_id)?;
    validate_identifier(&intake.participant_id)?;

    let room_name = format!("matter-{}", intake.matter_id);
    let object_key = format!("matters/{}/session-recording.webm", intake.matter_id);
    let idempotency_key = format!("{}-session-recording", intake.matter_id);
    let room = infrai
        .create_room(&CreateRoom {
            name: &room_name,
            max_participants: 8,
            empty_timeout_s: 900,
            region: "auto",
        })
        .await?;
    let room_token = infrai
        .issue_room_token(&IssueRoomToken {
            room: &room.name,
            identity: &intake.participant_id,
            display_name: &intake.participant_name,
            ttl_s: 3600,
            can_publish: true,
            can_subscribe: true,
        })
        .await?;
    let upload = infrai
        .presign_upload(
            bucket,
            &object_key,
            &PresignPut {
                op: "put",
                expires_seconds: 3600,
                content_type: "video/webm",
                max_bytes: 2_000_000_000,
                idempotency_key: &idempotency_key,
            },
        )
        .await?;

    Ok(SessionHandoff {
        matter_id: intake.matter_id,
        room: room.name,
        room_token: room_token.token,
        recording_upload_url: upload.url,
        recording_object_key: object_key,
        follow_up_in_days: follow_up_days(intake.signed_document_delivered),
    })
}

fn validate_identifier(value: &str) -> Result<(), IntakeError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    valid.then_some(()).ok_or(IntakeError::InvalidInput(
        "matter and participant identifiers must use 1-64 ASCII letters, digits, '-' or '_'",
    ))
}

#[cfg(test)]
mod tests {
    use super::follow_up_days;

    #[test]
    fn unsigned_delivery_gets_the_earlier_follow_up() {
        assert_eq!(follow_up_days(false), 2);
        assert_eq!(follow_up_days(true), 7);
    }
}
