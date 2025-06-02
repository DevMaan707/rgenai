use std::collections::HashMap;

use crate::{
    config::UpstashConfig,
    error::{BedrockError, Result},
    models::storage::{
        DeleteResult, InsertResult, UpdateResult, VectorInsert, VectorRecord, VectorSearch,
        VectorSearchResponse, VectorSearchResult, VectorUpdate,
    },
    storage::traits::{StorageStats, VectorStorage},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde_json::{json, Value};

use uuid::Uuid;

pub struct UpstashVectorStorage {
    client: Client,
    base_url: String,
    token: String,
}

impl UpstashVectorStorage {
    pub async fn new(config: UpstashConfig) -> Result<Self> {
        let base_url = config
            .url
            .ok_or_else(|| BedrockError::ConfigError("Upstash URL is required".into()))?;

        let token = config
            .token
            .ok_or_else(|| BedrockError::ConfigError("Upstash token is required".into()))?;

        let storage = Self {
            client: Client::new(),
            base_url,
            token,
        };

        // Test connection
        storage.health_check().await?;

        Ok(storage)
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.token).parse().unwrap(),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers
    }
}

#[async_trait]
impl VectorStorage for UpstashVectorStorage {
    async fn insert(&self, record: VectorInsert) -> Result<InsertResult> {
        let id = record.id.unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut metadata = record.metadata.clone();
        if let Some(content) = &record.content {
            metadata.insert("content".to_string(), json!(content));
        }
        if let Some(namespace) = &record.namespace {
            metadata.insert("namespace".to_string(), json!(namespace));
        }
        metadata.insert("created_at".to_string(), json!(Utc::now().to_rfc3339()));

        let payload = json!({
            "id": id,
            "vector": record.vector,
            "metadata": metadata
        });

        let response = self
            .client
            .post(&format!("{}/upsert", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Upstash request failed: {}", e)))?;

        if response.status().is_success() {
            Ok(InsertResult {
                id,
                success: true,
                message: Some("Vector inserted successfully".to_string()),
            })
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Ok(InsertResult {
                id,
                success: false,
                message: Some(format!("Insert failed: {}", error_text)),
            })
        }
    }

    async fn insert_batch(&self, records: Vec<VectorInsert>) -> Result<Vec<InsertResult>> {
        let vectors: Vec<Value> = records
            .iter()
            .map(|record| {
                let id = record
                    .id
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| Uuid::new_v4().to_string());

                let mut metadata = record.metadata.clone();
                if let Some(content) = &record.content {
                    metadata.insert("content".to_string(), json!(content));
                }
                if let Some(namespace) = &record.namespace {
                    metadata.insert("namespace".to_string(), json!(namespace));
                }
                metadata.insert("created_at".to_string(), json!(Utc::now().to_rfc3339()));

                json!({
                    "id": id,
                    "vector": record.vector,
                    "metadata": metadata
                })
            })
            .collect();

        let payload = json!({
            "vectors": vectors
        });

        let response = self
            .client
            .post(&format!("{}/upsert-batch", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                BedrockError::RequestError(format!("Upstash batch request failed: {}", e))
            })?;

        let mut results = Vec::new();

        if response.status().is_success() {
            for record in records {
                let id = record.id.unwrap_or_else(|| Uuid::new_v4().to_string());
                results.push(InsertResult {
                    id,
                    success: true,
                    message: Some("Vector inserted successfully".to_string()),
                });
            }
        } else {
            let error_text = response.text().await.unwrap_or_default();
            for record in records {
                let id = record.id.unwrap_or_else(|| Uuid::new_v4().to_string());
                results.push(InsertResult {
                    id,
                    success: false,
                    message: Some(format!("Batch insert failed: {}", error_text)),
                });
            }
        }

        Ok(results)
    }

    async fn search(&self, query: VectorSearch) -> Result<VectorSearchResponse> {
        let mut payload = json!({
            "vector": query.vector,
            "topK": query.limit,
            "includeMetadata": query.include_metadata,
            "includeVectors": query.include_content
        });

        if let Some(filter) = query.filter {
            payload["filter"] = json!(filter);
        }

        let response = self
            .client
            .post(&format!("{}/query", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Upstash search failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(BedrockError::RequestError(format!(
                "Search failed: {}",
                error_text
            )));
        }

        let response_json: Value = response.json().await.map_err(|e| {
            BedrockError::ResponseError(format!("Failed to parse search response: {}", e))
        })?;

        let matches = response_json["result"]
            .as_array()
            .ok_or_else(|| BedrockError::ResponseError("Invalid search response format".into()))?;

        let mut results = Vec::new();
        for match_item in matches {
            let metadata: HashMap<String, serde_json::Value> = match_item["metadata"]
                .as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            let content = if query.include_content {
                metadata
                    .get("content")
                    .and_then(|v| v.as_str())
                    .map(String::from)
            } else {
                None
            };

            results.push(VectorSearchResult {
                id: match_item["id"].as_str().unwrap_or("").to_string(),
                score: match_item["score"].as_f64().unwrap_or(0.0) as f32,
                vector: if query.include_content {
                    match_item["vector"].as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_f64().map(|f| f as f32))
                            .collect()
                    })
                } else {
                    None
                },
                metadata,
                content,
            });
        }

        Ok(VectorSearchResponse {
            total: results.len(),
            results,
        })
    }

    async fn get(&self, id: &str, _namespace: Option<&str>) -> Result<Option<VectorRecord>> {
        let payload = json!({
            "ids": [id],
            "includeMetadata": true,
            "includeVectors": true
        });

        let response = self
            .client
            .post(&format!("{}/fetch", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Upstash fetch failed: {}", e)))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let response_json: Value = response.json().await.map_err(|e| {
            BedrockError::ResponseError(format!("Failed to parse fetch response: {}", e))
        })?;

        let result = response_json["result"]
            .as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| BedrockError::ResponseError("No vector found".into()))?;

        if result.is_null() {
            return Ok(None);
        }

        let metadata: HashMap<String, serde_json::Value> = result["metadata"]
            .as_object()
            .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        let content = metadata
            .get("content")
            .and_then(|v| v.as_str())
            .map(String::from);
        let namespace = metadata
            .get("namespace")
            .and_then(|v| v.as_str())
            .map(String::from);
        let created_at_str = metadata
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let created_at = DateTime::parse_from_rfc3339(created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let vector = result["vector"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Some(VectorRecord {
            id: id.to_string(),
            vector,
            metadata,
            content,
            namespace,
            created_at,
            updated_at: created_at,
        }))
    }

    async fn update(&self, update: VectorUpdate) -> Result<UpdateResult> {
        // Get existing record
        let existing = self.get(&update.id, update.namespace.as_deref()).await?;

        if let Some(mut existing_record) = existing {
            // Update fields if provided
            if let Some(vector) = update.vector {
                existing_record.vector = vector;
            }
            if let Some(metadata) = update.metadata {
                existing_record.metadata.extend(metadata);
            }
            if let Some(content) = update.content {
                existing_record.content = Some(content);
                existing_record
                    .metadata
                    .insert("content".to_string(), json!(existing_record.content));
            }
            if let Some(namespace) = update.namespace {
                existing_record.namespace = Some(namespace);
            }

            existing_record
                .metadata
                .insert("updated_at".to_string(), json!(Utc::now().to_rfc3339()));

            let insert_record = VectorInsert {
                id: Some(existing_record.id.clone()),
                vector: existing_record.vector,
                metadata: existing_record.metadata,
                content: existing_record.content,
                namespace: existing_record.namespace,
            };

            let insert_result = self.insert(insert_record).await?;
            Ok(UpdateResult {
                id: update.id,
                success: insert_result.success,
                message: Some("Vector updated successfully".to_string()),
            })
        } else {
            Ok(UpdateResult {
                id: update.id,
                success: false,
                message: Some("Vector not found".to_string()),
            })
        }
    }

    async fn delete(&self, id: &str, _namespace: Option<&str>) -> Result<DeleteResult> {
        let payload = json!({
            "ids": [id]
        });

        let response = self
            .client
            .delete(&format!("{}/delete", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Upstash delete failed: {}", e)))?;

        Ok(DeleteResult {
            id: id.to_string(),
            success: response.status().is_success(),
            message: if response.status().is_success() {
                Some("Vector deleted successfully".to_string())
            } else {
                Some(format!("Delete failed: {}", response.status()))
            },
        })
    }

    async fn delete_batch(
        &self,
        ids: Vec<String>,
        _namespace: Option<&str>,
    ) -> Result<Vec<DeleteResult>> {
        let payload = json!({
            "ids": ids
        });

        let response = self
            .client
            .delete(&format!("{}/delete", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                BedrockError::RequestError(format!("Upstash batch delete failed: {}", e))
            })?;

        let success = response.status().is_success();
        let message = if success {
            "Vectors deleted successfully".to_string()
        } else {
            format!("Batch delete failed: {}", response.status())
        };

        Ok(ids
            .into_iter()
            .map(|id| DeleteResult {
                id,
                success,
                message: Some(message.clone()),
            })
            .collect())
    }

    async fn list(
        &self,
        _namespace: Option<&str>,
        _limit: Option<usize>,
    ) -> Result<Vec<VectorRecord>> {
        // Upstash doesn't have a direct list operation
        log::warn!("List operation not supported by Upstash - consider using search instead");
        Ok(vec![])
    }

    async fn stats(&self, _namespace: Option<&str>) -> Result<StorageStats> {
        let response = self
            .client
            .get(&format!("{}/info", self.base_url))
            .headers(self.build_headers())
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Upstash stats failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(BedrockError::RequestError("Failed to get stats".into()));
        }

        let stats_json: Value = response.json().await.map_err(|e| {
            BedrockError::ResponseError(format!("Failed to parse stats response: {}", e))
        })?;

        let total_vectors = stats_json["vectorCount"].as_u64().unwrap_or(0) as usize;
        let dimensions = stats_json["dimension"].as_u64().map(|d| d as usize);

        Ok(StorageStats {
            total_vectors,
            namespaces: vec!["default".to_string()], // Upstash doesn't use namespaces
            dimensions,
            storage_size_bytes: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(&format!("{}/info", self.base_url))
            .headers(self.build_headers())
            .send()
            .await
            .map_err(|_| BedrockError::InternalError("Health check failed".into()))?;

        Ok(response.status().is_success())
    }
}
