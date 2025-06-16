use crate::{
    error::{BedrockError, Result},
    models::{StreamChunk, TextGenerationRequest},
    ModelProvider,
};
use aws_sdk_bedrockruntime::{error::ProvideErrorMetadata, primitives::Blob, Client};
use futures::stream::Stream;
use serde_json::json;
use std::pin::Pin;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Clone)]
pub struct TextClient {
    client: Client,
}

impl TextClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn generate(&self, request: TextGenerationRequest) -> Result<String> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-text-express-v1");

        let request_payload = match request.provider.unwrap_or(ModelProvider::Amazon) {
            ModelProvider::Amazon => json!({
                "inputText": request.prompt,
                "textGenerationConfig": {
                    "maxTokenCount": request.max_tokens.unwrap_or(512),
                    "temperature": request.temperature.unwrap_or(0.7),
                    "topP": 0.9
                }
            }),
            ModelProvider::Anthropic => json!({
                "messages": [
                    {
                        "role": "user",
                        "content": request.prompt
                    }
                ],
                "max_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "anthropic_version": "bedrock-2023-05-31"
            }),
            ModelProvider::Cohere => json!({
                "prompt": request.prompt,
                "max_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "p": 0.9
            }),
            ModelProvider::AI21 => json!({
                "prompt": request.prompt,
                "maxTokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "topP": 0.9
            }),
            ModelProvider::Meta | ModelProvider::Mistral => json!({
                "prompt": request.prompt,
                "max_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "top_p": 0.9
            }),
        };
        let request_json = serde_json::to_string(&request_payload)
            .map_err(|e| BedrockError::SerializationError(e.to_string()))?;

        log::info!("Invoking model: {}", model_id);
        log::debug!("Text generation request payload: {}", request_json);

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
                log::error!("AWS SDK Text Generation Error details: {:?}", e);

                if let Some(service_error) = e.as_service_error() {
                    log::error!("Service error code: {:?}", service_error.code());
                    log::error!("Service error message: {:?}", service_error.message());
                    BedrockError::AwsServiceError(format!(
                        "Bedrock service error: {} - {}",
                        service_error.code().unwrap_or("unknown"),
                        service_error.message().unwrap_or("no message")
                    ))
                } else {
                    BedrockError::AwsError(format!("AWS SDK error: {}", e))
                }
            })?;

        let response_bytes = response.body.into_inner();
        String::from_utf8(response_bytes).map_err(|e| BedrockError::ResponseError(e.to_string()))
    }

    pub async fn generate_stream(
        &self,
        request: TextGenerationRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send>>> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-text-express-v1");

        let mut request_payload = self.build_request_payload(&request, model_id)?;
        match model_id {
            id if id.starts_with("amazon.titan") => {
                if let Some(obj) = request_payload.as_object_mut() {
                    if let Some(config) = obj.get_mut("textGenerationConfig") {
                        if let Some(config_obj) = config.as_object_mut() {
                            config_obj.insert("stream".to_string(), json!(true));
                        }
                    }
                }
            }
            id if id.starts_with("anthropic.claude") => {
                if let Some(obj) = request_payload.as_object_mut() {
                    obj.insert("stream".to_string(), json!(true));
                }
            }
            _ => {}
        }

        let request_json = serde_json::to_string(&request_payload)
            .map_err(|e| BedrockError::SerializationError(e.to_string()))?;

        log::info!("Invoking streaming model: {}", model_id);

        let response = self
            .client
            .invoke_model_with_response_stream()
            .model_id(model_id)
            .content_type("application/json")
            .accept("application/json")
            .body(Blob::new(request_json.into_bytes()))
            .send()
            .await
            .map_err(|e| BedrockError::AwsError(e.to_string()))?;

        let model_id = model_id.to_string();

        // Convert EventReceiver to a Stream using a channel
        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let mut event_receiver = response.body;

        tokio::spawn(async move {
            loop {
                match event_receiver.recv().await {
                    Ok(Some(event)) => {
                        let result = match event {
                            aws_sdk_bedrockruntime::types::ResponseStream::Chunk(chunk) => {
                                if let Some(bytes) = chunk.bytes {
                                    let chunk_str =
                                        String::from_utf8_lossy(bytes.as_ref()).to_string();
                                    Self::parse_stream_chunk_static(&chunk_str, &model_id)
                                } else {
                                    Ok(StreamChunk {
                                        chunk: String::new(),
                                        done: false,
                                        finish_reason: None,
                                    })
                                }
                            }
                            _ => Ok(StreamChunk {
                                chunk: String::new(),
                                done: true,
                                finish_reason: Some("complete".to_string()),
                            }),
                        };

                        if tx.send(result).await.is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = tx.send(Err(BedrockError::AwsError(e.to_string()))).await;
                        break;
                    }
                }
            }
        });

        Ok(Box::pin(ReceiverStream::new(rx)))
    }

    fn build_request_payload(
        &self,
        request: &TextGenerationRequest,
        model_id: &str,
    ) -> Result<serde_json::Value> {
        let payload = match model_id {
            id if id.starts_with("amazon.titan") => json!({
                "inputText": request.prompt,
                "textGenerationConfig": {
                    "maxTokenCount": request.max_tokens.unwrap_or(512),
                    "temperature": request.temperature.unwrap_or(0.7),
                    "topP": 0.9
                }
            }),
            id if id.starts_with("meta.llama") => json!({
                "prompt": request.prompt,
                "max_gen_len": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "top_p": 0.9
            }),
            id if id.starts_with("mistral.mistral") => json!({
                "prompt": request.prompt,
                "max_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "top_p": 0.9
            }),
            id if id.starts_with("arn:aws:bedrock") => json!({
                "messages": [
                    {
                        "role": "user",
                        "content": request.prompt
                    }
                ],
                "max_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "anthropic_version": "bedrock-2023-05-31"
            }),
            id if id.starts_with("anthropic.claude") => json!({
                "messages": [
                    {
                        "role": "user",
                        "content": request.prompt
                    }
                ],
                "max_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "anthropic_version": "bedrock-2023-05-31"
            }),
            id if id.starts_with("ai21.") => json!({
                "prompt": request.prompt,
                "maxTokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "topP": 0.9
            }),
            id if id.starts_with("cohere.command") => json!({
                "prompt": request.prompt,
                "max_tokens": request.max_tokens.unwrap_or(512),
                "temperature": request.temperature.unwrap_or(0.7),
                "p": 0.9
            }),
            _ => {
                return Err(BedrockError::RequestError(format!(
                    "Unsupported model ID: {}",
                    model_id
                )))
            }
        };

        Ok(payload)
    }

    fn parse_stream_chunk_static(chunk_str: &str, model_id: &str) -> Result<StreamChunk> {
        let json: serde_json::Value = serde_json::from_str(chunk_str)
            .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

        let stream_chunk = match model_id {
            id if id.starts_with("amazon.titan") => StreamChunk {
                chunk: json["outputText"].as_str().unwrap_or("").to_string(),
                done: json["completionReason"].is_string(),
                finish_reason: json["completionReason"].as_str().map(String::from),
            },
            id if id.starts_with("meta.llama") => StreamChunk {
                chunk: json["generation"].as_str().unwrap_or("").to_string(),
                done: json["stop_reason"].is_string(),
                finish_reason: json["stop_reason"].as_str().map(String::from),
            },
            id if id.starts_with("mistral.mistral") => StreamChunk {
                chunk: json["outputs"][0]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),
                done: json["outputs"][0]["stop_reason"].is_string(),
                finish_reason: json["outputs"][0]["stop_reason"].as_str().map(String::from),
            },
            id if id.starts_with("anthropic.claude") => {
                let delta = &json["delta"];
                StreamChunk {
                    chunk: delta["text"].as_str().unwrap_or("").to_string(),
                    done: json["type"].as_str() == Some("message_stop"),
                    finish_reason: json["delta"]["stop_reason"].as_str().map(String::from),
                }
            }
            _ => {
                return Err(BedrockError::ResponseError(
                    "Unexpected model type in streaming response".into(),
                ))
            }
        };

        Ok(stream_chunk)
    }
}
