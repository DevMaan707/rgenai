use crate::{
    error::{BedrockError, Result},
    models::{
        CohereEmbeddingRequest, CohereEmbeddingResponse, EmbeddingRequest, EmbeddingResponse,
        TitanEmbeddingResponse,
    },
};
use aws_sdk_bedrockruntime::{primitives::Blob, Client};
use serde_json::json;

#[derive(Clone)]
pub struct VectorClient {
    client: Client,
}

impl VectorClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn generate_embedding(&self, request: EmbeddingRequest) -> Result<EmbeddingResponse> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-embed-text-v2:0");

        let request_payload = match model_id {
            id if id.starts_with("amazon.titan-embed") => json!({
                "inputText": request.text,
            }),
            id if id.starts_with("cohere.embed") => {
                let cohere_request = CohereEmbeddingRequest {
                    texts: vec![request.text.clone()],
                    input_type: "search_document".to_string(),
                };
                serde_json::to_value(cohere_request)
                    .map_err(|e| BedrockError::SerializationError(e.to_string()))?
            }
            _ => {
                return Err(BedrockError::RequestError(
                    "Unsupported embedding model".into(),
                ))
            }
        };

        let request_json = serde_json::to_string(&request_payload)
            .map_err(|e| BedrockError::SerializationError(e.to_string()))?;

        log::info!("Generating embedding with model: {}", model_id);
        log::debug!("Embedding request payload: {}", request_json);

        let response = self
            .client
            .invoke_model()
            .model_id(model_id)
            .content_type("application/json")
            .accept("application/json")
            .body(Blob::new(request_json.into_bytes()))
            .send()
            .await
            .map_err(|e| {
                log::error!("AWS SDK Embedding Error details: {:?}", e);

                if let Some(service_error) = e.as_service_error() {
                    BedrockError::AwsServiceError(format!(
                        "Embedding service error: {:?}",
                        service_error
                    ))
                } else {
                    BedrockError::AwsError(format!("Embedding error: {}", e))
                }
            })?;

        let response_bytes = response.body.into_inner();
        let response_str = String::from_utf8(response_bytes)
            .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

        log::debug!("Embedding raw response: {}", response_str);

        let embedding = match model_id {
            id if id.starts_with("amazon.titan-embed") => {
                let titan_response: TitanEmbeddingResponse = serde_json::from_str(&response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;
                titan_response.embedding
            }
            id if id.starts_with("cohere.embed") => {
                let cohere_response: CohereEmbeddingResponse = serde_json::from_str(&response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                if cohere_response.embeddings.is_empty() {
                    return Err(BedrockError::ResponseError("No embeddings returned".into()));
                }

                cohere_response.embeddings[0].clone()
            }
            _ => {
                return Err(BedrockError::ResponseError(
                    "Unknown embedding model".into(),
                ))
            }
        };

        if embedding.is_empty() {
            return Err(BedrockError::ResponseError(
                "Empty embedding returned".into(),
            ));
        }

        Ok(EmbeddingResponse {
            embedding,
            model: model_id.to_string(),
        })
    }
}
