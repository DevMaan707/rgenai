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

    /// Get list of supported models with their proper IDs
    pub fn supported_models() -> Vec<(String, String, String)> {
        vec![
            // Amazon Titan models
            (
                "amazon.titan-text-express-v1".to_string(),
                "Amazon Titan Text Express".to_string(),
                "Amazon".to_string(),
            ),
            (
                "amazon.titan-text-lite-v1".to_string(),
                "Amazon Titan Text Lite".to_string(),
                "Amazon".to_string(),
            ),
            (
                "amazon.titan-text-premier-v1:0".to_string(),
                "Amazon Titan Text Premier".to_string(),
                "Amazon".to_string(),
            ),
            // Anthropic Claude models
            (
                "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string(),
                "Claude 3.5 Sonnet".to_string(),
                "Anthropic".to_string(),
            ),
            (
                "anthropic.claude-3-sonnet-20240229-v1:0".to_string(),
                "Claude 3 Sonnet".to_string(),
                "Anthropic".to_string(),
            ),
            (
                "anthropic.claude-3-haiku-20240307-v1:0".to_string(),
                "Claude 3 Haiku".to_string(),
                "Anthropic".to_string(),
            ),
            (
                "anthropic.claude-3-opus-20240229-v1:0".to_string(),
                "Claude 3 Opus".to_string(),
                "Anthropic".to_string(),
            ),
            (
                "anthropic.claude-v2:1".to_string(),
                "Claude 2.1".to_string(),
                "Anthropic".to_string(),
            ),
            (
                "anthropic.claude-instant-v1".to_string(),
                "Claude Instant".to_string(),
                "Anthropic".to_string(),
            ),
            // Meta Llama models
            (
                "meta.llama2-13b-chat-v1".to_string(),
                "Llama 2 13B Chat".to_string(),
                "Meta".to_string(),
            ),
            (
                "meta.llama2-70b-chat-v1".to_string(),
                "Llama 2 70B Chat".to_string(),
                "Meta".to_string(),
            ),
            (
                "meta.llama3-8b-instruct-v1:0".to_string(),
                "Llama 3 8B Instruct".to_string(),
                "Meta".to_string(),
            ),
            (
                "meta.llama3-70b-instruct-v1:0".to_string(),
                "Llama 3 70B Instruct".to_string(),
                "Meta".to_string(),
            ),
            (
                "meta.llama3-1-8b-instruct-v1:0".to_string(),
                "Llama 3.1 8B Instruct".to_string(),
                "Meta".to_string(),
            ),
            (
                "meta.llama3-1-70b-instruct-v1:0".to_string(),
                "Llama 3.1 70B Instruct".to_string(),
                "Meta".to_string(),
            ),
            (
                "meta.llama3-1-405b-instruct-v1:0".to_string(),
                "Llama 3.1 405B Instruct".to_string(),
                "Meta".to_string(),
            ),
            // Mistral models
            (
                "mistral.mistral-7b-instruct-v0:2".to_string(),
                "Mistral 7B Instruct".to_string(),
                "Mistral".to_string(),
            ),
            (
                "mistral.mixtral-8x7b-instruct-v0:1".to_string(),
                "Mixtral 8x7B Instruct".to_string(),
                "Mistral".to_string(),
            ),
            (
                "mistral.mistral-large-2402-v1:0".to_string(),
                "Mistral Large".to_string(),
                "Mistral".to_string(),
            ),
            (
                "mistral.mistral-large-2407-v1:0".to_string(),
                "Mistral Large 2407".to_string(),
                "Mistral".to_string(),
            ),
            // AI21 models
            (
                "ai21.j2-ultra-v1".to_string(),
                "Jurassic-2 Ultra".to_string(),
                "AI21".to_string(),
            ),
            (
                "ai21.j2-mid-v1".to_string(),
                "Jurassic-2 Mid".to_string(),
                "AI21".to_string(),
            ),
            (
                "ai21.jamba-instruct-v1:0".to_string(),
                "Jamba Instruct".to_string(),
                "AI21".to_string(),
            ),
            // Cohere models
            (
                "cohere.command-text-v14".to_string(),
                "Command".to_string(),
                "Cohere".to_string(),
            ),
            (
                "cohere.command-light-text-v14".to_string(),
                "Command Light".to_string(),
                "Cohere".to_string(),
            ),
            (
                "cohere.command-r-v1:0".to_string(),
                "Command R".to_string(),
                "Cohere".to_string(),
            ),
            (
                "cohere.command-r-plus-v1:0".to_string(),
                "Command R+".to_string(),
                "Cohere".to_string(),
            ),
        ]
    }

    pub async fn generate(&self, request: TextGenerationRequest) -> Result<TextGenerationResponse> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-text-express-v1");

        // Validate model ID
        if !Self::is_model_supported(model_id) {
            log::warn!(
                "Model '{}' may not be supported. Available models:",
                model_id
            );
            for (id, name, provider) in Self::supported_models() {
                log::info!("  {} - {} ({})", id, name, provider);
            }
        }

        let request_payload = self.build_request_payload(&request, model_id)?;
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
                    BedrockError::AwsServiceError(format!(
                        "Text generation service error: {:?}",
                        service_error
                    ))
                } else {
                    BedrockError::AwsError(format!("Text generation error: {}", e))
                }
            })?;

        let response_bytes = response.body.into_inner();
        let response_str = String::from_utf8(response_bytes)
            .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

        log::debug!("Text generation raw response: {}", response_str);

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

        // Add streaming configuration based on model type
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

    fn is_model_supported(model_id: &str) -> bool {
        Self::supported_models()
            .iter()
            .any(|(id, _, _)| id == model_id)
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

    fn parse_response(&self, response_str: &str, model_id: &str) -> Result<TextGenerationResponse> {
        match model_id {
            id if id.starts_with("amazon.titan") => {
                let titan_response: TitanTextResponse = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                let text_length = titan_response.output_text.len();
                let estimated_tokens = (text_length as f32 / 4.0).ceil() as i32;

                Ok(TextGenerationResponse {
                    text: titan_response.output_text,
                    model: model_id.to_string(),
                    tokens_generated: estimated_tokens,
                    tokens_prompt: 0,
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
            id if id.starts_with("anthropic.claude") => {
                let claude_response: serde_json::Value = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                let content = claude_response["content"][0]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                let usage = &claude_response["usage"];
                let input_tokens = usage["input_tokens"].as_i64().unwrap_or(0) as i32;
                let output_tokens = usage["output_tokens"].as_i64().unwrap_or(0) as i32;

                Ok(TextGenerationResponse {
                    text: content,
                    model: model_id.to_string(),
                    tokens_generated: output_tokens,
                    tokens_prompt: input_tokens,
                    finish_reason: claude_response["stop_reason"].as_str().map(String::from),
                })
            }
            id if id.starts_with("ai21.") => {
                let ai21_response: serde_json::Value = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                let completions = ai21_response["completions"]
                    .as_array()
                    .ok_or_else(|| BedrockError::ResponseError("No completions found".into()))?;

                if completions.is_empty() {
                    return Err(BedrockError::ResponseError(
                        "Empty completions array".into(),
                    ));
                }

                let text = completions[0]["data"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                Ok(TextGenerationResponse {
                    text,
                    model: model_id.to_string(),
                    tokens_generated: ai21_response["prompt"]["tokens"]
                        .as_array()
                        .map(|a| a.len() as i32)
                        .unwrap_or(0),
                    tokens_prompt: 0,
                    finish_reason: completions[0]["finishReason"]["reason"]
                        .as_str()
                        .map(String::from),
                })
            }
            id if id.starts_with("cohere.command") => {
                let cohere_response: serde_json::Value = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                let generations = cohere_response["generations"]
                    .as_array()
                    .ok_or_else(|| BedrockError::ResponseError("No generations found".into()))?;

                if generations.is_empty() {
                    return Err(BedrockError::ResponseError(
                        "Empty generations array".into(),
                    ));
                }

                let text = generations[0]["text"].as_str().unwrap_or("").to_string();

                Ok(TextGenerationResponse {
                    text,
                    model: model_id.to_string(),
                    tokens_generated: 0, // Cohere doesn't always provide token counts
                    tokens_prompt: 0,
                    finish_reason: generations[0]["finish_reason"].as_str().map(String::from),
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
