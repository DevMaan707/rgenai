pub mod pinecone;
#[cfg(feature = "postgres")]
pub mod postgres;
pub mod traits;
pub mod upstash;

use crate::{config::Config, error::Result};
use std::sync::Arc;
use traits::VectorStorage;

#[cfg(feature = "postgres")]
use postgres::PostgresVectorStorage;

#[cfg(feature = "pinecone")]
use pinecone::PineconeVectorStorage;

#[cfg(feature = "upstash")]
use upstash::UpstashVectorStorage;

pub use traits::{StorageStats, VectorStorage as VectorStorageTrait};

pub struct VectorStorageManager {
    backend: Arc<dyn VectorStorage>,
}

impl VectorStorageManager {
    pub async fn new(config: Config) -> Result<Self> {
        let backend: Arc<dyn VectorStorage> = if config.use_psql {
            #[cfg(feature = "postgres")]
            {
                let postgres_config = config.postgres.ok_or_else(|| {
                    crate::error::BedrockError::ConfigError("PostgreSQL config required".into())
                })?;
                Arc::new(PostgresVectorStorage::new(postgres_config).await?)
            }
            #[cfg(not(feature = "postgres"))]
            {
                return Err(crate::error::BedrockError::ConfigError(
                    "PostgreSQL feature not enabled".into(),
                ));
            }
        } else if config.use_pinecone {
            #[cfg(feature = "pinecone")]
            {
                let pinecone_config = config.pinecone.ok_or_else(|| {
                    crate::error::BedrockError::ConfigError("Pinecone config required".into())
                })?;
                Arc::new(PineconeVectorStorage::new(pinecone_config).await?)
            }
            #[cfg(not(feature = "pinecone"))]
            {
                return Err(crate::error::BedrockError::ConfigError(
                    "Pinecone feature not enabled".into(),
                ));
            }
        } else if config.use_upstash {
            #[cfg(feature = "upstash")]
            {
                let upstash_config = config.upstash.ok_or_else(|| {
                    crate::error::BedrockError::ConfigError("Upstash config required".into())
                })?;
                Arc::new(UpstashVectorStorage::new(upstash_config).await?)
            }
            #[cfg(not(feature = "upstash"))]
            {
                return Err(crate::error::BedrockError::ConfigError(
                    "Upstash feature not enabled".into(),
                ));
            }
        } else {
            return Err(crate::error::BedrockError::ConfigError(
                "No storage backend configured".into(),
            ));
        };

        Ok(Self { backend })
    }

    pub fn storage(&self) -> &Arc<dyn VectorStorage> {
        &self.backend
    }
}
impl VectorStorageManager {
    pub async fn insert(
        &self,
        record: crate::models::storage::VectorInsert,
    ) -> Result<crate::models::storage::InsertResult> {
        self.backend.insert(record).await
    }

    pub async fn insert_batch(
        &self,
        records: Vec<crate::models::storage::VectorInsert>,
    ) -> Result<Vec<crate::models::storage::InsertResult>> {
        self.backend.insert_batch(records).await
    }

    pub async fn search(
        &self,
        query: crate::models::storage::VectorSearch,
    ) -> Result<crate::models::storage::VectorSearchResponse> {
        self.backend.search(query).await
    }

    pub async fn get(
        &self,
        id: &str,
        namespace: Option<&str>,
    ) -> Result<Option<crate::models::storage::VectorRecord>> {
        self.backend.get(id, namespace).await
    }

    pub async fn update(
        &self,
        update: crate::models::storage::VectorUpdate,
    ) -> Result<crate::models::storage::UpdateResult> {
        self.backend.update(update).await
    }

    pub async fn delete(
        &self,
        id: &str,
        namespace: Option<&str>,
    ) -> Result<crate::models::storage::DeleteResult> {
        self.backend.delete(id, namespace).await
    }

    pub async fn delete_batch(
        &self,
        ids: Vec<String>,
        namespace: Option<&str>,
    ) -> Result<Vec<crate::models::storage::DeleteResult>> {
        self.backend.delete_batch(ids, namespace).await
    }

    pub async fn list(
        &self,
        namespace: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<crate::models::storage::VectorRecord>> {
        self.backend.list(namespace, limit).await
    }

    pub async fn stats(&self, namespace: Option<&str>) -> Result<StorageStats> {
        self.backend.stats(namespace).await
    }

    pub async fn health_check(&self) -> Result<bool> {
        self.backend.health_check().await
    }
}
