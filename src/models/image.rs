use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub model_id: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub num_images: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct ImageGenerationResponse {
    pub image_data: String, // Base64 encoded
    pub model: String,
}

#[derive(Serialize, Deserialize)]
pub struct TitanImageResponse {
    pub images: Vec<String>,
}
