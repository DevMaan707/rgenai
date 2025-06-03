use crate::{
    error::{BedrockError, Result},
    models::{ImageGenerationRequest, ImageGenerationResponse, TitanImageResponse},
};
use aws_sdk_bedrockruntime::{primitives::Blob, Client};
use serde_json::json;

#[derive(Clone)]
pub struct ImageClient {
    client: Client,
}

impl ImageClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// Get list of supported image models
    pub fn supported_models() -> Vec<(String, String, String)> {
        vec![
            (
                "amazon.titan-image-generator-v1".to_string(),
                "Titan Image Generator".to_string(),
                "Amazon".to_string(),
            ),
            (
                "amazon.titan-image-generator-v2:0".to_string(),
                "Titan Image Generator V2".to_string(),
                "Amazon".to_string(),
            ),
            (
                "stability.stable-diffusion-xl-v1:0".to_string(),
                "Stable Diffusion XL".to_string(),
                "Stability AI".to_string(),
            ),
        ]
    }

    pub async fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-image-generator-v1");

        let request_payload = match model_id {
            id if id.starts_with("amazon.titan-image-generator") => json!({
                "taskType": "TEXT_IMAGE",
                "textToImageParams": {
                    "text": request.prompt,
                    "width": request.width.unwrap_or(1024),
                    "height": request.height.unwrap_or(1024)
                },
                "imageGenerationConfig": {
                    "numberOfImages": request.num_images.unwrap_or(1),
                    "quality": "standard",
                    "cfgScale": 8.0
                }
            }),
            id if id.starts_with("stability.stable-diffusion") => json!({
                "text_prompts": [
                    {
                        "text": request.prompt,
                        "weight": 1.0
                    }
                ],
                "cfg_scale": 10,
                "seed": 0,
                "steps": 50,
                "width": request.width.unwrap_or(1024),
                "height": request.height.unwrap_or(1024),
                "samples": request.num_images.unwrap_or(1)
            }),
            _ => {
                return Err(BedrockError::RequestError(format!(
                    "Unsupported image model: {}",
                    model_id
                )))
            }
        };

        let request_json = serde_json::to_string(&request_payload)
            .map_err(|e| BedrockError::SerializationError(e.to_string()))?;

        log::info!("Generating image with model: {}", model_id);
        log::debug!("Image request payload: {}", request_json);

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

        log::debug!("Image generation raw response: {}", response_str);

        self.parse_response(&response_str, model_id)
    }

    fn parse_response(
        &self,
        response_str: &str,
        model_id: &str,
    ) -> Result<ImageGenerationResponse> {
        match model_id {
            id if id.starts_with("amazon.titan-image-generator") => {
                let titan_response: TitanImageResponse = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                if titan_response.images.is_empty() {
                    return Err(BedrockError::ResponseError("No images generated".into()));
                }

                Ok(ImageGenerationResponse {
                    image_data: titan_response.images[0].clone(),
                    model: model_id.to_string(),
                })
            }
            id if id.starts_with("stability.") => {
                let stability_response: serde_json::Value = serde_json::from_str(response_str)
                    .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

                let artifacts = stability_response["artifacts"]
                    .as_array()
                    .ok_or_else(|| BedrockError::ResponseError("No artifacts found".into()))?;

                if artifacts.is_empty() {
                    return Err(BedrockError::ResponseError("No images generated".into()));
                }

                let image_data = artifacts[0]["base64"]
                    .as_str()
                    .ok_or_else(|| BedrockError::ResponseError("No image data found".into()))?
                    .to_string();

                Ok(ImageGenerationResponse {
                    image_data,
                    model: model_id.to_string(),
                })
            }
            _ => Err(BedrockError::ResponseError(
                "Unknown image model type".into(),
            )),
        }
    }
}
