use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddingRequest {
    pub text: String,
    pub model_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct EmbeddingResponse {
    pub embedding: Vec<f32>,
    pub model: String,
}

#[derive(Serialize, Deserialize)]
pub struct TitanEmbeddingResponse {
    pub embedding: Vec<f32>,
}

#[derive(Serialize, Deserialize)]
pub struct CohereEmbeddingRequest {
    pub texts: Vec<String>,
    pub input_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct CohereEmbeddingResponse {
    pub embeddings: Vec<Vec<f32>>,
}
