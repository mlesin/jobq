use crate::{Job, JobRequest, Priority};
use anyhow::Error;
use log::debug;
use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Executor;
use std::sync::Arc;

#[derive(Clone)]
pub struct DbHandle {
    pool: Arc<PgPool>,
}

impl DbHandle {
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

    pub(crate) async fn complete_job(&self, id: i64) -> Result<(), Error> {
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

    pub(crate) async fn fail_job(&self, id: i64, msg: String) -> Result<(), Error> {
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

    pub(crate) async fn begin_job(&self, id: i64) -> Result<(), Error> {
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

    pub(crate) async fn get_processing_jobs(&self) -> Result<Vec<Job>, Error> {
        debug!("Getting processing jobs");
        Ok(sqlx::query_as!(
            Job,
            "SELECT id, name, username, uuid, params, priority as \"priority: _\", status as \"status: _\" \
            FROM jobq \
            WHERE status = 'PROCESSING' \
            ORDER BY priority asc, started_at asc"
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(crate) async fn get_queued_jobs(&self, num: i64) -> Result<Vec<Job>, Error> {
        debug!("Getting {} queued jobs", num);
        Ok(sqlx::query_as!(
            Job,
            "SELECT id, name, username, uuid, params, priority as \"priority: _\", status as \"status: _\" \
            FROM jobq \
            WHERE status = 'QUEUED' \
            ORDER BY priority asc, started_at asc \
            LIMIT $1",
            &num
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub(crate) async fn submit_job_request(&self, job: &JobRequest) -> Result<i64, Error> {
        debug!("Submitting job {:?}", job);
        let result = sqlx::query_scalar!(
            "INSERT INTO jobq \
            (name, username, uuid, params, priority, status) \
            VALUES ($1, $2, $3, $4, $5, 'QUEUED') \
            RETURNING id",
            &job.name,
            &job.username,
            &job.uuid,
            &job.params,
            &job.priority as &Priority
        )
        .fetch_one(&*self.pool)
        .await?;

        Ok(result)
    }
}
