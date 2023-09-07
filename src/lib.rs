use uuid::Uuid;

pub mod db;
pub mod server;
pub mod telemetry;
pub mod worker;

#[derive(Debug)]
pub struct JobRequest {
    pub project_id: Uuid,
    pub post_id: Uuid,
    pub filename: String,
    pub hash: String,
    pub mimetype: String,
    pub sort_order: i16,
}

#[derive(Debug, Clone, sqlx::FromRow)]
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

#[derive(Debug, Clone, sqlx::Type)]
#[sqlx(type_name = "queue_status_enum", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Queued,
    Processing,
    Completed,
    Failed,
}
