use serde_derive::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub mod db;
pub mod dealer;
pub mod router;
pub mod server;
pub mod worker;

#[derive(Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Hello(String),
    Request(JobRequest),
    Completed(Job),
    Failed(Job, String),
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ClientMessage {
    Hello(String),
    Order(Job),
    Acknowledged(Job),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JobRequest {
    pub name: String,
    pub username: String,
    pub uuid: Uuid,
    pub params: Value,
    pub priority: Priority,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::FromRow)]
pub struct Job {
    pub id: i64,
    pub username: String,
    pub name: String,
    pub uuid: Uuid,
    pub params: Value,
    pub priority: Priority,
    pub status: Status,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::Type)]
#[sqlx(type_name = "status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Queued,
    Processing,
    Completed,
    Failed,
}

#[derive(Serialize, Deserialize, Debug, Clone, sqlx::Type)]
#[sqlx(type_name = "priority_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Priority {
    High,
    Normal,
    Low,
}
