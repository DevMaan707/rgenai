#[cfg(feature = "postgres")]
use crate::{
    config::PostgresConfig,
    error::{BedrockError, Result},
    models::storage::{
        DeleteResult, InsertResult, UpdateResult, VectorInsert, VectorRecord, VectorSearch,
        VectorSearchResponse, VectorSearchResult, VectorUpdate,
    },
    storage::traits::{StorageStats, VectorStorage},
};

#[cfg(feature = "postgres")]
use async_trait::async_trait;
#[cfg(feature = "postgres")]
use chrono::{DateTime, Utc};
#[cfg(feature = "postgres")]
use deadpool_postgres::{Config, Pool, Runtime};
#[cfg(feature = "postgres")]
use pgvector::Vector;
#[cfg(feature = "postgres")]
use std::collections::HashMap;
#[cfg(feature = "postgres")]
use tokio_postgres::{types::ToSql, NoTls};
#[cfg(feature = "postgres")]
use uuid::Uuid;

#[cfg(feature = "postgres")]
pub struct PostgresVectorStorage {
    pool: Pool,
}

#[cfg(feature = "postgres")]
impl PostgresVectorStorage {
    pub async fn new(config: PostgresConfig) -> Result<Self> {
        let mut cfg = Config::new();
        cfg.host = config.host;
        cfg.port = config.port;
        cfg.user = config.username;
        cfg.password = config.password;
        cfg.dbname = config.database;

        let pool = cfg
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| BedrockError::ConfigError(format!("Failed to create pool: {}", e)))?;

        let storage = Self { pool };
        storage.initialize_schema().await?;

        Ok(storage)
    }

    async fn initialize_schema(&self) -> Result<()> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        // Create pgvector extension
        client
            .execute("CREATE EXTENSION IF NOT EXISTS vector", &[])
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to create vector extension: {}", e))
            })?;
        client
            .execute(
                "CREATE TABLE IF NOT EXISTS vectors (
                id TEXT PRIMARY KEY,
                vector VECTOR,
                metadata JSONB DEFAULT '{}',
                content TEXT,
                namespace TEXT DEFAULT 'default',
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            )",
                &[],
            )
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to create vectors table: {}", e))
            })?;

        client
            .execute(
                "CREATE INDEX IF NOT EXISTS idx_vectors_namespace ON vectors(namespace)",
                &[],
            )
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to create namespace index: {}", e))
            })?;
        let _ = client.execute(
            "CREATE INDEX IF NOT EXISTS idx_vectors_vector ON vectors USING ivfflat (vector vector_cosine_ops) WITH (lists = 100)",
            &[],
        ).await;

        log::info!("PostgreSQL vector storage schema initialized");
        Ok(())
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl VectorStorage for PostgresVectorStorage {
    async fn insert(&self, record: VectorInsert) -> Result<InsertResult> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        let id = record.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let vector = Vector::from(record.vector);
        let namespace = record.namespace.as_deref().unwrap_or("default");
        let metadata = serde_json::to_value(&record.metadata)
            .map_err(|e| BedrockError::SerializationError(e.to_string()))?;

        let stmt = client
            .prepare(
                "INSERT INTO vectors (id, vector, metadata, content, namespace, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
             ON CONFLICT (id) DO UPDATE SET
                vector = EXCLUDED.vector,
                metadata = EXCLUDED.metadata,
                content = EXCLUDED.content,
                namespace = EXCLUDED.namespace,
                updated_at = NOW()",
            )
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare statement: {}", e))
            })?;

        client
            .execute(
                &stmt,
                &[&id, &vector, &metadata, &record.content, &namespace],
            )
            .await
            .map_err(|e| BedrockError::InternalError(format!("Failed to insert vector: {}", e)))?;

        Ok(InsertResult {
            id,
            success: true,
            message: Some("Vector inserted successfully".to_string()),
        })
    }

    async fn insert_batch(&self, records: Vec<VectorInsert>) -> Result<Vec<InsertResult>> {
        let mut results = Vec::new();

        for record in records {
            let result = self.insert(record).await;
            match result {
                Ok(success_result) => results.push(success_result),
                Err(e) => results.push(InsertResult {
                    id: "unknown".to_string(),
                    success: false,
                    message: Some(e.to_string()),
                }),
            }
        }

        Ok(results)
    }

    async fn search(&self, query: VectorSearch) -> Result<VectorSearchResponse> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        let query_vector = Vector::from(query.vector);
        let namespace = query.namespace.as_deref().unwrap_or("default");
        let limit = query.limit as i64;

        let stmt = client
            .prepare(
                "SELECT id, vector, metadata, content, 1 - (vector <=> $1) as similarity
             FROM vectors
             WHERE namespace = $2
             ORDER BY vector <=> $1
             LIMIT $3",
            )
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare search statement: {}", e))
            })?;

        let rows = client
            .query(&stmt, &[&query_vector, &namespace, &limit])
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to execute search query: {}", e))
            })?;

        let mut results = Vec::new();
        for row in rows {
            let vector: Option<Vector> = if query.include_content {
                Some(row.get("vector"))
            } else {
                None
            };
            let metadata: serde_json::Value = row.get("metadata");
            let metadata_map: HashMap<String, serde_json::Value> =
                serde_json::from_value(metadata).unwrap_or_default();

            results.push(VectorSearchResult {
                id: row.get("id"),
                score: row.get("similarity"),
                vector: vector.map(|v| v.to_vec()),
                metadata: metadata_map,
                content: if query.include_content {
                    row.get("content")
                } else {
                    None
                },
            });
        }

        Ok(VectorSearchResponse {
            total: results.len(),
            results,
        })
    }

    async fn get(&self, id: &str, namespace: Option<&str>) -> Result<Option<VectorRecord>> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        let namespace = namespace.unwrap_or("default");

        let stmt = client
            .prepare(
                "SELECT id, vector, metadata, content, namespace, created_at, updated_at
             FROM vectors WHERE id = $1 AND namespace = $2",
            )
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare get statement: {}", e))
            })?;

        let rows = client.query(&stmt, &[&id, &namespace]).await.map_err(|e| {
            BedrockError::InternalError(format!("Failed to execute get query: {}", e))
        })?;

        if rows.is_empty() {
            return Ok(None);
        }

        let row = &rows[0];
        let vector: Vector = row.get("vector");
        let metadata: serde_json::Value = row.get("metadata");
        let metadata_map: HashMap<String, serde_json::Value> =
            serde_json::from_value(metadata).unwrap_or_default();

        let created_at: DateTime<Utc> = row.get("created_at");
        let updated_at: DateTime<Utc> = row.get("updated_at");

        Ok(Some(VectorRecord {
            id: row.get("id"),
            vector: vector.to_vec(),
            metadata: metadata_map,
            content: row.get("content"),
            namespace: Some(row.get("namespace")),
            created_at,
            updated_at,
        }))
    }

    async fn update(&self, update: VectorUpdate) -> Result<UpdateResult> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        let mut set_clauses = Vec::new();
        let mut params: Vec<Box<dyn ToSql + Send + Sync>> = vec![Box::new(update.id.clone())];
        let mut param_count = 1;

        if let Some(vector) = &update.vector {
            param_count += 1;
            set_clauses.push(format!("vector = ${}", param_count));
            params.push(Box::new(Vector::from(vector.clone())));
        }

        if let Some(metadata) = &update.metadata {
            param_count += 1;
            set_clauses.push(format!("metadata = ${}", param_count));
            let metadata_value = serde_json::to_value(metadata)
                .map_err(|e| BedrockError::SerializationError(e.to_string()))?;
            params.push(Box::new(metadata_value));
        }

        if let Some(content) = &update.content {
            param_count += 1;
            set_clauses.push(format!("content = ${}", param_count));
            params.push(Box::new(content.clone()));
        }

        if let Some(namespace) = &update.namespace {
            param_count += 1;
            set_clauses.push(format!("namespace = ${}", param_count));
            params.push(Box::new(namespace.clone()));
        }

        if set_clauses.is_empty() {
            return Ok(UpdateResult {
                id: update.id,
                success: false,
                message: Some("No fields to update".to_string()),
            });
        }

        set_clauses.push("updated_at = NOW()".to_string());

        let query = format!(
            "UPDATE vectors SET {} WHERE id = $1",
            set_clauses.join(", ")
        );

        let stmt = client.prepare(&query).await.map_err(|e| {
            BedrockError::InternalError(format!("Failed to prepare update statement: {}", e))
        })?;

        let param_refs: Vec<&(dyn ToSql + Sync)> = params.iter().map(|p| p.as_ref()).collect();

        let rows_affected = client
            .execute(&stmt, &param_refs)
            .await
            .map_err(|e| BedrockError::InternalError(format!("Failed to execute update: {}", e)))?;

        Ok(UpdateResult {
            id: update.id,
            success: rows_affected > 0,
            message: if rows_affected > 0 {
                Some("Vector updated successfully".to_string())
            } else {
                Some("Vector not found".to_string())
            },
        })
    }

    async fn delete(&self, id: &str, namespace: Option<&str>) -> Result<DeleteResult> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        let namespace = namespace.unwrap_or("default");

        let stmt = client
            .prepare("DELETE FROM vectors WHERE id = $1 AND namespace = $2")
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare delete statement: {}", e))
            })?;

        let rows_affected = client
            .execute(&stmt, &[&id, &namespace])
            .await
            .map_err(|e| BedrockError::InternalError(format!("Failed to execute delete: {}", e)))?;

        Ok(DeleteResult {
            id: id.to_string(),
            success: rows_affected > 0,
            message: if rows_affected > 0 {
                Some("Vector deleted successfully".to_string())
            } else {
                Some("Vector not found".to_string())
            },
        })
    }

    async fn delete_batch(
        &self,
        ids: Vec<String>,
        namespace: Option<&str>,
    ) -> Result<Vec<DeleteResult>> {
        let mut results = Vec::new();

        for id in ids {
            let result = self.delete(&id, namespace).await;
            match result {
                Ok(delete_result) => results.push(delete_result),
                Err(e) => results.push(DeleteResult {
                    id,
                    success: false,
                    message: Some(e.to_string()),
                }),
            }
        }

        Ok(results)
    }

    async fn list(
        &self,
        namespace: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<VectorRecord>> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        let namespace = namespace.unwrap_or("default");
        let limit = limit.unwrap_or(100) as i64;

        let stmt = client
            .prepare(
                "SELECT id, vector, metadata, content, namespace, created_at, updated_at
             FROM vectors WHERE namespace = $1 ORDER BY created_at DESC LIMIT $2",
            )
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare list statement: {}", e))
            })?;

        let rows = client
            .query(&stmt, &[&namespace, &limit])
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to execute list query: {}", e))
            })?;

        let mut records = Vec::new();
        for row in rows {
            let vector: Vector = row.get("vector");
            let metadata: serde_json::Value = row.get("metadata");
            let metadata_map: HashMap<String, serde_json::Value> =
                serde_json::from_value(metadata).unwrap_or_default();

            let created_at: DateTime<Utc> = row.get("created_at");
            let updated_at: DateTime<Utc> = row.get("updated_at");

            records.push(VectorRecord {
                id: row.get("id"),
                vector: vector.to_vec(),
                metadata: metadata_map,
                content: row.get("content"),
                namespace: Some(row.get("namespace")),
                created_at,
                updated_at,
            });
        }

        Ok(records)
    }

    async fn stats(&self, namespace: Option<&str>) -> Result<StorageStats> {
        let client =
            self.pool.get().await.map_err(|e| {
                BedrockError::InternalError(format!("Failed to get connection: {}", e))
            })?;

        let namespace = namespace.unwrap_or("default");
        let count_stmt = client
            .prepare("SELECT COUNT(*) FROM vectors WHERE namespace = $1")
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare count statement: {}", e))
            })?;

        let count_row = client
            .query_one(&count_stmt, &[&namespace])
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to execute count query: {}", e))
            })?;

        let total_vectors: i64 = count_row.get(0);
        let ns_stmt = client
            .prepare("SELECT DISTINCT namespace FROM vectors")
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare namespace statement: {}", e))
            })?;

        let ns_rows = client.query(&ns_stmt, &[]).await.map_err(|e| {
            BedrockError::InternalError(format!("Failed to execute namespace query: {}", e))
        })?;

        let namespaces: Vec<String> = ns_rows.iter().map(|row| row.get(0)).collect();
        let dim_stmt = client
            .prepare(
                "SELECT array_length(vector, 1) as dimensions FROM vectors WHERE namespace = $1 LIMIT 1",
            )
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to prepare dimension statement: {}", e))
            })?;

        let dimensions = client
            .query(&dim_stmt, &[&namespace])
            .await
            .map_err(|e| {
                BedrockError::InternalError(format!("Failed to execute dimension query: {}", e))
            })?
            .first()
            .and_then(|row| row.get::<_, Option<i32>>(0))
            .map(|d| d as usize);

        Ok(StorageStats {
            total_vectors: total_vectors as usize,
            namespaces,
            dimensions,
            storage_size_bytes: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let client = self
            .pool
            .get()
            .await
            .map_err(|_| BedrockError::InternalError("Failed to get connection".into()))?;

        client
            .execute("SELECT 1", &[])
            .await
            .map_err(|_| BedrockError::InternalError("Health check query failed".into()))?;

        Ok(true)
    }
}
#[cfg(not(feature = "postgres"))]
pub struct PostgresVectorStorage;

#[cfg(not(feature = "postgres"))]
impl PostgresVectorStorage {
    pub async fn new(_config: crate::config::PostgresConfig) -> crate::error::Result<Self> {
        Err(crate::error::BedrockError::ConfigError(
            "PostgreSQL feature not enabled".into(),
        ))
    }
}
