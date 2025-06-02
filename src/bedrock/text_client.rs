use crate::{
    error::{BedrockError, Result},
    models::{
        LlamaResponse, StreamChunk, TextGenerationRequest, TextGenerationResponse,
        TitanTextResponse,
    },
};
use aws_sdk_bedrockruntime::{primitives::Blob, Client};
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

    pub async fn generate(&self, request: TextGenerationRequest) -> Result<TextGenerationResponse> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-text-express-v1");

        let request_payload = self.build_request_payload(&request, model_id)?;
        let request_json = serde_json::to_string(&request_payload)
            .map_err(|e| BedrockError::SerializationError(e.to_string()))?;

        log::info!("Invoking model: {}", model_id);

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
        let response_str = String::from_utf8(response_bytes)
            .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

        self.parse_response(&response_str, model_id)
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

        // Add streaming configuration
        if model_id.starts_with("amazon.titan") {
            if let Some(obj) = request_payload.as_object_mut() {
                obj.insert("stream".to_string(), json!(true));
            }
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
            _ => return Err(BedrockError::RequestError("Unsupported model ID".into())),
        };

        Ok(payload)
    }

    fn parse_response(&self, response_str: &str, model_id: &str) -> Result<TextGenerationResponse> {
        match model_id {
            id if id.starts_with("amazon.titan") => {
                let titan_response: TitanTextResponse = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                // Calculate tokens before moving the text
                let text_length = titan_response.output_text.len();
                let estimated_tokens = (text_length as f32 / 4.0).ceil() as i32;

                Ok(TextGenerationResponse {
                    text: titan_response.output_text,
                    model: model_id.to_string(),
                    tokens_generated: estimated_tokens,
                    tokens_prompt: 0, // Titan doesn't provide this
                    finish_reason: titan_response.completion_reason,
                })
            }
            id if id.starts_with("meta.llama") || id.starts_with("mistral.mistral") => {
                let llama_response: LlamaResponse = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                Ok(TextGenerationResponse {
                    text: llama_response.generation,
                    model: model_id.to_string(),
                    tokens_generated: llama_response.generation_token_count,
                    tokens_prompt: llama_response.prompt_token_count,
                    finish_reason: Some(llama_response.stop_reason),
                })
            }
            _ => Err(BedrockError::ResponseError("Unknown model type".into())),
        }
    }

    // Static version for use in async context
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
            _ => {
                return Err(BedrockError::ResponseError(
                    "Unexpected model type in streaming response".into(),
                ))
            }
        };

        Ok(stream_chunk)
    }
}
