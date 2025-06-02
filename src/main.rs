use rgen::{BedrockClient, BedrockConfig};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file first
    match dotenv::dotenv() {
        Ok(_) => log::info!("✅ .env file loaded successfully"),
        Err(_) => log::warn!("⚠️  No .env file found, using system environment variables"),
    }

    rgen::logger::init_with_config(
        rgen::logger::LoggerConfig::development().with_level(rgen::logger::LogLevel::Debug),
    )?;

    log::info!("🔍 Checking AWS environment...");

    if let Ok(profile) = env::var("AWS_PROFILE") {
        log::info!("AWS_PROFILE: {}", profile);
    }

    if let Ok(region) = env::var("AWS_DEFAULT_REGION") {
        log::info!("AWS_DEFAULT_REGION: {}", region);
    } else if let Ok(region) = env::var("AWS_REGION") {
        log::info!("AWS_REGION: {}", region);
    } else {
        log::warn!("No AWS region environment variable set, using us-east-1");
    }

    // Check credentials (without printing the actual values for security)
    match (
        env::var("AWS_ACCESS_KEY_ID"),
        env::var("AWS_SECRET_ACCESS_KEY"),
    ) {
        (Ok(access_key), Ok(secret_key)) => {
            log::info!("✅ AWS credentials found in environment");
            log::debug!(
                "Access Key ID starts with: {}...",
                &access_key[..5.min(access_key.len())]
            );
            log::debug!("Secret Key length: {}", secret_key.len());
        }
        _ => {
            log::warn!("⚠️  No AWS credentials in environment variables, will try default credential chain");
            log::error!("❌ This will likely cause authentication failures");
        }
    }

    let config = BedrockConfig::new().with_region("us-east-1");

    log::info!("🔄 Creating Bedrock client...");
    let client = match BedrockClient::new(config).await {
        Ok(client) => {
            log::info!("✅ Bedrock client initialized successfully");
            client
        }
        Err(e) => {
            log::error!("❌ Failed to initialize Bedrock client: {}", e);
            return Err(e.into());
        }
    };

    log::info!("🔄 Testing text generation...");
    let text_request = rgen::TextGenerationRequest {
        prompt: "Write a short poem about AI".to_string(),
        max_tokens: Some(100),
        temperature: Some(0.7),
        model_id: Some("meta.llama4-scout-17b-instruct-v1:0".to_string()),
        stream: None,
    };

    match client.text().generate(text_request).await {
        Ok(response) => {
            log::info!("✅ Text generation successful!");
            log::info!("📝 Generated text: {}", response.text);
            log::info!("🔢 Tokens generated: {}", response.tokens_generated);
            log::info!("🤖 Model used: {}", response.model);
            if let Some(reason) = response.finish_reason {
                log::info!("🏁 Finish reason: {}", reason);
            }
        }
        Err(e) => {
            log::error!("❌ Text generation failed: {}", e);
            log::error!(
                "💡 Tip: Make sure you have access to Amazon Titan models in your AWS account"
            );
        }
    }

    // Test embedding generation
    log::info!("🔄 Testing embedding generation...");
    let embedding_request = rgen::EmbeddingRequest {
        text: "Hello, world!".to_string(),
        model_id: Some("amazon.titan-embed-text-v2:0".to_string()),
    };

    match client.vector().generate_embedding(embedding_request).await {
        Ok(response) => {
            log::info!("✅ Embedding generation successful!");
            log::info!(
                "📐 Generated embedding with {} dimensions",
                response.embedding.len()
            );
            log::info!("🤖 Model used: {}", response.model);
            log::debug!(
                "🔢 First 5 embedding values: {:?}",
                &response.embedding[..5.min(response.embedding.len())]
            );
        }
        Err(e) => {
            log::error!("❌ Embedding generation failed: {}", e);
            log::error!("💡 Tip: Make sure you have access to Amazon Titan Embedding models in your AWS account");
        }
    }

    log::info!("🎉 Library test completed!");

    Ok(())
}
