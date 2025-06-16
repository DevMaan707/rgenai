use crate::{
    error::{BedrockError, Result},
    models::EmbeddingRequest,
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

    pub async fn generate_embedding(&self, request: EmbeddingRequest) -> Result<String> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-embed-text-v1");
        let request_payload = json!({
            "inputText": request.text
        });
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
            .map_err(|e| BedrockError::AwsError(e.to_string()))?;

        let response_bytes = response.body.into_inner();
        String::from_utf8(response_bytes).map_err(|e| BedrockError::ResponseError(e.to_string()))
    }
}
