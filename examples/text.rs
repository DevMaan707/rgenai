use rgenai::{BedrockClient, BedrockConfig, ModelProvider, TextGenerationRequest};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match dotenv::dotenv() {
        Ok(_) => log::info!("✅ .env file loaded"),
        Err(_) => log::warn!("⚠️  No .env file found"),
    }
    rgenai::logger::init()?;
    let access_key = env::var("AWS_ACCESS_KEY_ID")?;
    let secret_key = env::var("AWS_SECRET_ACCESS_KEY")?;
    let region = env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
    let config = BedrockConfig::new()
        .with_region(&region)
        .with_credentials(access_key, secret_key);

    let client = BedrockClient::new(config).await?;
    let request = TextGenerationRequest {
        prompt: "create me a very well designed frontend for web app in next js which is responsive and mobile-friendly, and design is modern and minimalist, a clone to chatgpt , which gives calls to localserver and gets prompt output , which you should show properly. make sure to focus on design and look and feel!!".to_string(),
        max_tokens: Some(15000),
        temperature: Some(0.7),
        model_id: Some(
            "arn:aws:bedrock:us-east-1:022499029734:application-inference-profile/y3hjlp87ql6f"
                .to_string(),
        ),
        stream: None,
        provider: Some(ModelProvider::Anthropic)
    };

    let response = client.text().generate(request).await?;
    println!("{:?}", response);

    Ok(())
}
