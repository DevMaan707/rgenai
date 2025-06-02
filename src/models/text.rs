use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct TextGenerationRequest {
    pub prompt: String,
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    pub model_id: Option<String>,
    pub stream: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct TextGenerationResponse {
    pub text: String,
    pub model: String,
    pub tokens_generated: i32,
    pub tokens_prompt: i32,
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct LlamaResponse {
    pub generation: String,
    pub prompt_token_count: i32,
    pub generation_token_count: i32,
    pub stop_reason: String,
}

#[derive(Serialize, Deserialize)]
pub struct TitanTextResponse {
    #[serde(rename = "outputText")]
    pub output_text: String,
    #[serde(rename = "completionReason")]
    pub completion_reason: Option<String>,
}
