use anyhow::Error;
use serde_derive::{Deserialize, Serialize};
use tmq::{Message, Multipart};
use uuid::Uuid;

pub mod db;
pub mod server;
pub mod telemetry;
pub mod worker;

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Hello,
    Request(JobRequest),
    Completed(Job),
    Failed(Job, String),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Hello,
    Acknowledged(Job),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum WorkerMessage {
    Hello,
    Order(Job),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobRequest {
    pub project_id: Uuid,
    pub post_id: Uuid,
    pub filename: String,
    pub hash: String,
    pub mimetype: String,
    pub sort_order: i16,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub project_id: Uuid,
    pub post_id: Uuid,
    pub filename: String,
    pub hash: String,
    pub mimetype: String,
    pub sort_order: i16,
    pub status: Status,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::Type)]
#[sqlx(type_name = "queue_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Queued,
    Processing,
    Completed,
    Failed,
}

pub trait ToMpart {
    fn to_mpart(&self) -> Result<Multipart, Error>;

    fn to_msg(&self) -> Result<Message, Error>;
}

impl<T: serde::ser::Serialize> ToMpart for T {
    fn to_mpart(&self) -> Result<Multipart, Error> {
        let bytes = serde_cbor::to_vec(&self)?;

        Ok(Multipart::from(vec![&bytes]))
    }

    fn to_msg(&self) -> Result<Message, Error> {
        let bytes = serde_cbor::to_vec(&self)?;

        Ok(Message::from(&bytes))
    }
}
