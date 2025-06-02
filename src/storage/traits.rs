use crate::{
    error::Result,
    models::storage::{
        DeleteResult, InsertResult, UpdateResult, VectorInsert, VectorRecord, VectorSearch,
        VectorSearchResponse, VectorUpdate,
    },
};
use async_trait::async_trait;

#[async_trait]
pub trait VectorStorage: Send + Sync {
    async fn insert(&self, record: VectorInsert) -> Result<InsertResult>;
    async fn insert_batch(&self, records: Vec<VectorInsert>) -> Result<Vec<InsertResult>>;
    async fn search(&self, query: VectorSearch) -> Result<VectorSearchResponse>;
    async fn get(&self, id: &str, namespace: Option<&str>) -> Result<Option<VectorRecord>>;
    async fn update(&self, update: VectorUpdate) -> Result<UpdateResult>;

    async fn delete(&self, id: &str, namespace: Option<&str>) -> Result<DeleteResult>;

    async fn delete_batch(
        &self,
        ids: Vec<String>,
        namespace: Option<&str>,
    ) -> Result<Vec<DeleteResult>>;

    async fn list(
        &self,
        namespace: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<VectorRecord>>;
    async fn stats(&self, namespace: Option<&str>) -> Result<StorageStats>;

    async fn health_check(&self) -> Result<bool>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StorageStats {
    pub total_vectors: usize,
    pub namespaces: Vec<String>,
    pub dimensions: Option<usize>,
    pub storage_size_bytes: Option<u64>,
}
