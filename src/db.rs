use crate::{Job, JobRequest};
use anyhow::Error;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Executor;
use std::sync::Arc;
use tracing::instrument;
use uuid::Uuid;

#[derive(Clone)]
pub struct DbHandle {
    pool: Arc<PgPool>,
}

impl DbHandle {
    // #[instrument(name = "db.new")]
    pub(crate) async fn new(url: &str) -> Result<Self, Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&url)
            .await?;

        (&pool).execute(include_str!("setup.sql")).await?;

        Ok(DbHandle {
            pool: Arc::new(pool),
        })
    }

    #[instrument(name = "db.set_completed", skip_all, fields(job_id = %id))]
    pub(crate) async fn complete_job(&self, id: Uuid) -> Result<(), Error> {
        sqlx::query!(
            "UPDATE jobq \
                SET status = 'COMPLETED', \
                duration = extract(epoch from now() - started_at) \
            WHERE id = $1",
            &id
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    #[instrument(name = "db.set_failed", skip_all, fields(job_id = %id, error = %msg))]
    pub(crate) async fn fail_job(&self, id: Uuid, msg: String) -> Result<(), Error> {
        sqlx::query!(
            "UPDATE jobq \
                SET status = 'FAILED', \
                duration = extract(epoch from now() - started_at), \
                error = $1 \
            WHERE id = $2",
            &msg,
            &id
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    #[instrument(name = "db.set_processing", skip_all, fields(job_id = %id))]
    pub(crate) async fn begin_job(&self, id: Uuid) -> Result<(), Error> {
        sqlx::query!(
            "UPDATE jobq \
                SET status = 'PROCESSING', \
                started_at = now() \
            WHERE id = $1",
            &id
        )
        .execute(&*self.pool)
        .await?;

        Ok(())
    }

    #[instrument(name = "db.get_processing_jobs", skip_all)]
    pub(crate) async fn get_processing_jobs(&self) -> Result<Vec<Job>, Error> {
        // debug!("Getting processing jobs");
        Ok(sqlx::query_as!(
            Job,
            "SELECT id, project_id, post_id, filename, hash, mimetype, sort_order, status as \"status: _\" \
            FROM jobq \
            WHERE status = 'PROCESSING' \
            ORDER BY started_at asc"
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    #[instrument(name = "db.get_queued_jobs", skip_all, fields(limit = %num))]
    pub(crate) async fn get_queued_jobs(&self, num: i64) -> Result<Vec<Job>, Error> {
        // debug!("Getting {} queued jobs", num);
        Ok(sqlx::query_as!(
            Job,
            "SELECT id, project_id, post_id, filename, hash, mimetype, sort_order, status as \"status: _\" \
            FROM jobq \
            WHERE status = 'QUEUED' \
            ORDER BY started_at asc \
            LIMIT $1",
            &num
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    #[instrument(name = "db.submit_job_request", skip_all, fields(job_id))]
    pub(crate) async fn submit_job_request(&self, job: &JobRequest) -> Result<Uuid, Error> {
        // debug!("Submitting job");
        let result = sqlx::query_scalar!(
            "INSERT INTO jobq \
            (project_id, post_id, filename, hash, mimetype, sort_order, status) \
            VALUES ($1, $2, $3, $4, $5, $6, 'QUEUED') \
            RETURNING id",
            &job.project_id,
            &job.post_id,
            &job.filename,
            &job.hash,
            &job.mimetype,
            &job.sort_order
        )
        .fetch_one(&*self.pool)
        .await?;

        tracing::Span::current().record("job_id", &result.to_string());
        Ok(result)
    }
}
