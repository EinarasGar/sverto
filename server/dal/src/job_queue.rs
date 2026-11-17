use apalis::prelude::{Task, TaskSink, TaskSinkError};
use apalis_postgres::sink::push_tasks;
use apalis_postgres::{Config, PgContext, PgTask, PostgresStorage};
use serde::{de::DeserializeOwned, Serialize};
use sqlx::{PgConnection, PgPool};

#[derive(Clone)]
pub struct JobQueueHandle {
    pool: PgPool,
}

impl JobQueueHandle {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
        let mut migrator = PostgresStorage::<(), (), ()>::migrations();
        migrator.set_ignore_missing(true);
        migrator.run(pool).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub async fn push<T>(&self, job: T) -> anyhow::Result<()>
    where
        T: Serialize + DeserializeOwned + Send + Sync + Unpin + 'static,
    {
        let mut storage = PostgresStorage::<T>::new(&self.pool);
        storage.push(job).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(key = key))]
    pub async fn push_once<T>(&self, job: T, key: &str) -> anyhow::Result<()>
    where
        T: Serialize + DeserializeOwned + Send + Sync + Unpin + 'static,
    {
        let mut storage = PostgresStorage::<T>::new(&self.pool);
        let task = Task::builder(job).with_idempotency_key(key).build();
        match storage.push_task(task).await {
            Ok(()) => Ok(()),
            Err(TaskSinkError::PushError(sqlx::Error::Database(error)))
                if error.is_unique_violation() =>
            {
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    #[tracing::instrument(level = "debug", skip_all, fields(max_attempts = max_attempts))]
    pub async fn push_with_max_attempts<T>(&self, job: T, max_attempts: i32) -> anyhow::Result<()>
    where
        T: Serialize + DeserializeOwned + Send + Sync + Unpin + 'static,
    {
        let mut storage = PostgresStorage::<T>::new(&self.pool);
        let task = Task::builder(job)
            .with_ctx(PgContext::new().with_max_attempts(max_attempts))
            .build();
        storage.push_task(task).await?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all, fields(count = jobs.len()))]
    pub(crate) async fn push_all<T: Serialize>(
        &self,
        transaction: Option<&mut PgConnection>,
        jobs: Vec<T>,
    ) -> anyhow::Result<()> {
        if jobs.is_empty() {
            return Ok(());
        }
        let tasks = jobs
            .iter()
            .map(|job| Ok(Task::builder(serde_json::to_vec(job)?).build()))
            .collect::<anyhow::Result<Vec<PgTask<Vec<u8>>>>>()?;
        let config = Config::new(std::any::type_name::<T>());
        match transaction {
            Some(connection) => push_tasks(connection, config, tasks).await?,
            None => push_tasks(&self.pool, config, tasks).await?,
        }
        Ok(())
    }
}
