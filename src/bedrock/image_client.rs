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

    pub async fn generate(
        &self,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse> {
        let model_id = request
            .model_id
            .as_deref()
            .unwrap_or("amazon.titan-image-generator-v1");

        let request_payload = match model_id {
            "amazon.titan-image-generator-v1" => json!({
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
            _ => return Err(BedrockError::RequestError("Unsupported image model".into())),
        };

        let request_json = serde_json::to_string(&request_payload)
            .map_err(|e| BedrockError::SerializationError(e.to_string()))?;

        log::info!("Generating image with model: {}", model_id);

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

        let titan_response: TitanImageResponse = serde_json::from_str(&response_str)
            .map_err(|e| BedrockError::ResponseError(e.to_string()))?;

        if titan_response.images.is_empty() {
            return Err(BedrockError::ResponseError("No images generated".into()));
        }

        Ok(ImageGenerationResponse {
            image_data: titan_response.images[0].clone(),
            model: model_id.to_string(),
        })
    }
}
