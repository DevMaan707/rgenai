use std::collections::HashMap;

use crate::{
    config::PineconeConfig,
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

pub struct PineconeVectorStorage {
    client: Client,
    api_key: String,
    environment: String,
    index_name: String,
    base_url: String,
}

impl PineconeVectorStorage {
    pub async fn new(config: PineconeConfig) -> Result<Self> {
        let api_key = config
            .api_key
            .ok_or_else(|| BedrockError::ConfigError("Pinecone API key is required".into()))?;

        let environment = config
            .environment
            .ok_or_else(|| BedrockError::ConfigError("Pinecone environment is required".into()))?;

        let index_name = config
            .index_name
            .ok_or_else(|| BedrockError::ConfigError("Pinecone index name is required".into()))?;

        let base_url = format!(
            "https://{}-{}.svc.{}.pinecone.io",
            index_name, "PROJECT_ID", environment
        );

        let storage = Self {
            client: Client::new(),
            api_key,
            environment,
            index_name,
            base_url,
        };

        // Test connection
        storage.health_check().await?;

        Ok(storage)
    }

    fn build_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Api-Key", self.api_key.parse().unwrap());
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers
    }
}

#[async_trait]
impl VectorStorage for PineconeVectorStorage {
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
            "vectors": [{
                "id": id,
                "values": record.vector,
                "metadata": metadata
            }],
            "namespace": record.namespace.unwrap_or_else(|| "default".to_string())
        });

        let response = self
            .client
            .post(&format!("{}/vectors/upsert", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Pinecone request failed: {}", e)))?;

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
        if records.is_empty() {
            return Ok(vec![]);
        }

        let namespace = records
            .first()
            .and_then(|r| r.namespace.as_ref())
            .cloned()
            .unwrap_or_else(|| "default".to_string());

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
                metadata.insert("created_at".to_string(), json!(Utc::now().to_rfc3339()));

                json!({
                    "id": id,
                    "values": record.vector,
                    "metadata": metadata
                })
            })
            .collect();

        let payload = json!({
            "vectors": vectors,
            "namespace": namespace
        });
        let response = self
            .client
            .post(&format!("{}/vectors/upsert", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                BedrockError::RequestError(format!("Pinecone batch request failed: {}", e))
            })?;

        let mut results = Vec::new();

        if response.status().is_success() {
            // Pinecone returns success for the whole batch, so we assume all succeeded
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
        let payload = json!({
            "vector": query.vector,
            "topK": query.limit,
            "namespace": query.namespace.unwrap_or_else(|| "default".to_string()),
            "includeMetadata": query.include_metadata,
            "includeValues": query.include_content,
            "filter": query.filter.unwrap_or_default()
        });

        let response = self
            .client
            .post(&format!("{}/query", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Pinecone search failed: {}", e)))?;

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

        let matches = response_json["matches"]
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
                    match_item["values"].as_array().map(|arr| {
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

    async fn get(&self, id: &str, namespace: Option<&str>) -> Result<Option<VectorRecord>> {
        let namespace = namespace.unwrap_or("default");

        let payload = json!({
            "ids": [id],
            "namespace": namespace,
            "includeMetadata": true,
            "includeValues": true
        });

        let response = self
            .client
            .post(&format!("{}/vectors/fetch", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Pinecone fetch failed: {}", e)))?;

        if !response.status().is_success() {
            return Ok(None);
        }

        let response_json: Value = response.json().await.map_err(|e| {
            BedrockError::ResponseError(format!("Failed to parse fetch response: {}", e))
        })?;

        let vectors = response_json["vectors"]
            .as_object()
            .ok_or_else(|| BedrockError::ResponseError("Invalid fetch response format".into()))?;

        if let Some(vector_data) = vectors.get(id) {
            let metadata: HashMap<String, serde_json::Value> = vector_data["metadata"]
                .as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default();

            let content = metadata
                .get("content")
                .and_then(|v| v.as_str())
                .map(String::from);
            let created_at_str = metadata
                .get("created_at")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let created_at = DateTime::parse_from_rfc3339(created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let vector = vector_data["values"]
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
                namespace: Some(namespace.to_string()),
                created_at,
                updated_at: created_at, // Pinecone doesn't track update time separately
            }))
        } else {
            Ok(None)
        }
    }

    async fn update(&self, update: VectorUpdate) -> Result<UpdateResult> {
        // Pinecone updates via upsert
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

            // Add updated timestamp
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

    async fn delete(&self, id: &str, namespace: Option<&str>) -> Result<DeleteResult> {
        let namespace = namespace.unwrap_or("default");

        let payload = json!({
            "ids": [id],
            "namespace": namespace
        });

        let response = self
            .client
            .delete(&format!("{}/vectors/delete", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Pinecone delete failed: {}", e)))?;

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
        namespace: Option<&str>,
    ) -> Result<Vec<DeleteResult>> {
        let namespace = namespace.unwrap_or("default");

        let payload = json!({
            "ids": ids,
            "namespace": namespace
        });

        let response = self
            .client
            .delete(&format!("{}/vectors/delete", self.base_url))
            .headers(self.build_headers())
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                BedrockError::RequestError(format!("Pinecone batch delete failed: {}", e))
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
        namespace: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<VectorRecord>> {
        // Pinecone doesn't have a direct list operation, so we'd need to implement pagination
        // For now, return empty - this would require storing IDs separately or using describe_index_stats
        log::warn!(
            "List operation not efficiently supported by Pinecone - consider using search instead"
        );
        Ok(vec![])
    }

    async fn stats(&self, namespace: Option<&str>) -> Result<StorageStats> {
        let response = self
            .client
            .post(&format!("{}/describe_index_stats", self.base_url))
            .headers(self.build_headers())
            .json(&json!({}))
            .send()
            .await
            .map_err(|e| BedrockError::RequestError(format!("Pinecone stats failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(BedrockError::RequestError("Failed to get stats".into()));
        }

        let stats_json: Value = response.json().await.map_err(|e| {
            BedrockError::ResponseError(format!("Failed to parse stats response: {}", e))
        })?;

        let total_vectors = stats_json["totalVectorCount"].as_u64().unwrap_or(0) as usize;
        let dimensions = stats_json["dimension"].as_u64().map(|d| d as usize);

        let namespaces = stats_json["namespaces"]
            .as_object()
            .map(|ns| ns.keys().cloned().collect())
            .unwrap_or_else(|| vec!["default".to_string()]);

        Ok(StorageStats {
            total_vectors,
            namespaces,
            dimensions,
            storage_size_bytes: None,
        })
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .post(&format!("{}/describe_index_stats", self.base_url))
            .headers(self.build_headers())
            .json(&json!({}))
            .send()
            .await
            .map_err(|_| BedrockError::InternalError("Health check failed".into()))?;

        Ok(response.status().is_success())
    }
}
