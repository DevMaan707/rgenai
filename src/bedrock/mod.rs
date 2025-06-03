pub mod image_client;
pub mod text_client;
pub mod vector_client;

use crate::{
    config::{BedrockConfig, Config},
    error::Result,
    storage::VectorStorageManager,
    BedrockError,
};
use aws_sdk_bedrockruntime::Client;
use std::sync::Arc;

pub use image_client::ImageClient;
pub use text_client::TextClient;
pub use vector_client::VectorClient;

#[derive(Clone)]
pub struct BedrockClient {
    text_client: TextClient,
    image_client: ImageClient,
    vector_client: VectorClient,
    storage: Option<Arc<VectorStorageManager>>,
}

impl BedrockClient {
    pub async fn new(bedrock_config: BedrockConfig) -> Result<Self> {
        let aws_config = if let (Some(access_key), Some(secret_key)) =
            (&bedrock_config.access_key, &bedrock_config.secret_key)
        {
            aws_config::from_env()
                .credentials_provider(aws_sdk_bedrockruntime::config::Credentials::new(
                    access_key,
                    secret_key,
                    None,
                    None,
                    "bedrock-client",
                ))
                .region(aws_sdk_bedrockruntime::config::Region::new(
                    bedrock_config
                        .region
                        .unwrap_or_else(|| "us-east-1".to_string()),
                ))
                .load()
                .await
        } else {
            aws_config::load_from_env().await
        };

        let client = Client::new(&aws_config);

        Ok(Self {
            text_client: TextClient::new(client.clone()),
            image_client: ImageClient::new(client.clone()),
            vector_client: VectorClient::new(client.clone()),

            storage: None,
        })
    }

    pub async fn with_storage(
        bedrock_config: BedrockConfig,
        storage_config: Config,
    ) -> Result<Self> {
        let mut client = Self::new(bedrock_config).await?;

        let storage_manager = VectorStorageManager::new(storage_config).await?;
        client.storage = Some(Arc::new(storage_manager));

        Ok(client)
    }

    pub fn text(&self) -> &TextClient {
        &self.text_client
    }

    pub fn image(&self) -> &ImageClient {
        &self.image_client
    }

    pub fn vector(&self) -> &VectorClient {
        &self.vector_client
    }

    pub fn storage(&self) -> Option<&Arc<VectorStorageManager>> {
        self.storage.as_ref()
    }

    /// Generate embedding and optionally store it
    pub async fn embed_and_store(
        &self,
        text: &str,
        model_id: Option<&str>,
        metadata: Option<std::collections::HashMap<String, serde_json::Value>>,
        namespace: Option<&str>,
    ) -> Result<crate::models::storage::InsertResult> {
        // Generate embedding
        let embedding_request = crate::models::embedding::EmbeddingRequest {
            text: text.to_string(),
            model_id: model_id.map(String::from),
        };

        let embedding_response = self
            .vector_client
            .generate_embedding(embedding_request)
            .await?;

        // Store if storage is available
        if let Some(storage) = &self.storage {
            let insert_record = crate::models::storage::VectorInsert {
                id: None,
                vector: embedding_response.embedding,
                metadata: metadata.unwrap_or_default(),
                content: Some(text.to_string()),
                namespace: namespace.map(String::from),
            };

            storage.insert(insert_record).await
        } else {
            Err(BedrockError::ConfigError(
                "No storage backend configured".into(),
            ))
        }
    }

    pub async fn semantic_search(
        &self,
        query: &str,
        limit: usize,
        model_id: Option<&str>,
        namespace: Option<&str>,
        include_content: bool,
    ) -> Result<crate::models::storage::VectorSearchResponse> {
        let embedding_request = crate::models::embedding::EmbeddingRequest {
            text: query.to_string(),
            model_id: model_id.map(String::from),
        };

        let embedding_response = self
            .vector_client
            .generate_embedding(embedding_request)
            .await?;

        if let Some(storage) = &self.storage {
            let search_query = crate::models::storage::VectorSearch {
                vector: embedding_response.embedding,
                limit,
                namespace: namespace.map(String::from),
                filter: None,
                include_metadata: true,
                include_content,
            };

            storage.search(search_query).await
        } else {
            Err(BedrockError::ConfigError(
                "No storage backend configured".into(),
            ))
        }
    }

    /// RAG: Retrieve relevant context and generate response
    pub async fn generate_with_context(
        &self,
        query: &str,
        context_limit: usize,
        generation_model: Option<&str>,
        embedding_model: Option<&str>,
        namespace: Option<&str>,
        max_tokens: Option<i32>,
        temperature: Option<f32>,
    ) -> Result<String> {
        // 1. Search for relevant context
        let search_results = self
            .semantic_search(
                query,
                context_limit,
                embedding_model,
                namespace,
                true, // include content
            )
            .await?;

        // 2. Build context from search results
        let context: Vec<String> = search_results
            .results
            .iter()
            .filter_map(|result| result.content.as_ref())
            .cloned()
            .collect();

        if context.is_empty() {
            log::warn!("No relevant context found for query");
        }

        // 3. Build enhanced prompt with context
        let context_text = context.join("\n\n");
        let enhanced_prompt = if !context_text.is_empty() {
            format!(
                "Context:\n{}\n\nQuestion: {}\n\nAnswer based on the provided context:",
                context_text, query
            )
        } else {
            format!("Question: {}\n\nAnswer:", query)
        };

        // 4. Generate response
        let text_request = crate::models::text::TextGenerationRequest {
            prompt: enhanced_prompt,
            max_tokens,
            temperature,
            model_id: generation_model.map(String::from),
            stream: None,
        };

        let response = self.text_client.generate(text_request).await?;
        Ok(response.text)
    }
}
