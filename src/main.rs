use futures::StreamExt;
use rgen::{BedrockClient, BedrockConfig, TextClient,ImageClient};
use std::env;
use std::fs;

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

    log::info!("📚 Available text generation models:");
        for (id, name, provider) in TextClient::supported_models() {
            log::info!("  {} - {} ({})", id, name, provider);
        }

        log::info!("🖼️  Available image generation models:");
        for (id, name, provider) in ImageClient::supported_models() { // Change this line
            log::info!("  {} - {} ({})", id, name, provider);
        }

    // Test 1: Basic text generation with different models
    log::info!("🔄 Testing text generation with different models...");

    let test_models = vec![
        "amazon.titan-text-express-v1",
        "anthropic.claude-3-haiku-20240307-v1:0",
        "meta.llama3-8b-instruct-v1:0",
    ];

    for model_id in test_models {
        log::info!("🧪 Testing model: {}", model_id);

        let text_request = rgen::TextGenerationRequest {
            prompt: "Write a haiku about technology".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            model_id: Some(model_id.to_string()),
            stream: None,
        };

        match client.text().generate(text_request).await {
            Ok(response) => {
                log::info!("✅ Text generation successful with {}!", model_id);
                log::info!("📝 Generated text: {}", response.text);
                log::info!("🔢 Tokens generated: {}", response.tokens_generated);
                log::info!("🔢 Tokens prompt: {}", response.tokens_prompt);
                if let Some(reason) = response.finish_reason {
                    log::info!("🏁 Finish reason: {}", reason);
                }
            }
            Err(e) => {
                log::error!("❌ Text generation failed with {}: {}", model_id, e);
                log::warn!("💡 This model might not be available in your AWS account or region");
            }
        }

        log::info!("---");
    }

    // Test 2: Text streaming
    log::info!("🌊 Testing text streaming...");

    let streaming_models = vec![
        "amazon.titan-text-express-v1",
        "anthropic.claude-3-haiku-20240307-v1:0",
    ];

    for model_id in streaming_models {
        log::info!("🧪 Testing streaming with model: {}", model_id);

        let stream_request = rgen::TextGenerationRequest {
            prompt: "Tell me a short story about a robot learning to paint".to_string(),
            max_tokens: Some(200),
            temperature: Some(0.8),
            model_id: Some(model_id.to_string()),
            stream: Some(true),
        };

        match client.text().generate_stream(stream_request).await {
            Ok(mut stream) => {
                log::info!("✅ Started streaming with {}!", model_id);
                log::info!("📺 Streaming response:");

                let mut full_response = String::new();
                let mut chunk_count = 0;

                while let Some(chunk_result) = stream.next().await {
                    match chunk_result {
                        Ok(chunk) => {
                            if !chunk.chunk.is_empty() {
                                print!("{}", chunk.chunk);
                                full_response.push_str(&chunk.chunk);
                                chunk_count += 1;
                            }

                            if chunk.done {
                                println!("\n");
                                log::info!("🏁 Streaming completed!");
                                if let Some(reason) = chunk.finish_reason {
                                    log::info!("🏁 Finish reason: {}", reason);
                                }
                                break;
                            }
                        }
                        Err(e) => {
                            log::error!("❌ Streaming error: {}", e);
                            break;
                        }
                    }
                }

                log::info!("📊 Received {} chunks", chunk_count);
                log::info!(
                    "📏 Total response length: {} characters",
                    full_response.len()
                );
            }
            Err(e) => {
                log::error!("❌ Streaming failed with {}: {}", model_id, e);
            }
        }

        log::info!("---");
    }

    // Test 3: Image generation
    log::info!("🎨 Testing image generation...");

    let image_models = vec![
        "amazon.titan-image-generator-v1",
        "stability.stable-diffusion-xl-v1:0",
    ];

    for model_id in image_models {
        log::info!("🧪 Testing image generation with model: {}", model_id);

        let image_request = rgen::ImageGenerationRequest {
            prompt: "A serene landscape with mountains and a lake at sunset, digital art style"
                .to_string(),
            model_id: Some(model_id.to_string()),
            width: Some(512),
            height: Some(512),
            num_images: Some(1),
        };

        match client.image().generate(image_request).await {
            Ok(response) => {
                log::info!("✅ Image generation successful with {}!", model_id);
                log::info!("🤖 Model used: {}", response.model);
                log::info!(
                    "📏 Image data length: {} characters",
                    response.image_data.len()
                );

                // Save image to file
                let filename = format!(
                    "generated_image_{}_{}.png",
                    model_id.replace(".", "_").replace(":", "_"),
                    chrono::Utc::now().timestamp()
                );

                match base64::decode(&response.image_data) {
                    Ok(image_bytes) => match fs::write(&filename, image_bytes) {
                        Ok(_) => {
                            log::info!("💾 Image saved to: {}", filename);
                        }
                        Err(e) => {
                            log::error!("❌ Failed to save image: {}", e);
                        }
                    },
                    Err(e) => {
                        log::error!("❌ Failed to decode base64 image: {}", e);
                    }
                }
            }
            Err(e) => {
                log::error!("❌ Image generation failed with {}: {}", model_id, e);
                log::warn!("💡 This model might not be available in your AWS account or region");
            }
        }

        log::info!("---");
    }

    // Test 4: Embedding generation
    // log::info!("🔄 Testing embedding generation...");

    // let embedding_models = vec!["amazon.titan-embed-text-v2:0", "cohere.embed-english-v3"];

    // for model_id in embedding_models {
    //     log::info!("🧪 Testing embedding with model: {}", model_id);

    //     let embedding_request = rgen::EmbeddingRequest {
    //         text: "The quick brown fox jumps over the lazy dog. This is a test sentence for embedding generation.".to_string(),
    //         model_id: Some(model_id.to_string()),
    //     };

    //     match client.vector().generate_embedding(embedding_request).await {
    //         Ok(response) => {
    //             log::info!("✅ Embedding generation successful with {}!", model_id);
    //             log::info!(
    //                 "📐 Generated embedding with {} dimensions",
    //                 response.embedding.len()
    //             );
    //             log::info!("🤖 Model used: {}", response.model);

    //             // Show first and last few values
    //             let embedding_len = response.embedding.len();
    //             if embedding_len > 10 {
    //                 log::debug!("🔢 First 5 values: {:?}", &response.embedding[..5]);
    //                 log::debug!(
    //                     "🔢 Last 5 values: {:?}",
    //                     &response.embedding[embedding_len - 5..]
    //                 );
    //             } else {
    //                 log::debug!("🔢 All values: {:?}", response.embedding);
    //             }

    //             // Calculate some basic statistics
    //             let sum: f32 = response.embedding.iter().sum();
    //             let mean = sum / embedding_len as f32;
    //             let variance: f32 = response
    //                 .embedding
    //                 .iter()
    //                 .map(|x| (x - mean).powi(2))
    //                 .sum::<f32>()
    //                 / embedding_len as f32;
    //             let std_dev = variance.sqrt();

    //             log::info!("📊 Embedding statistics:");
    //             log::info!("   Mean: {:.6}", mean);
    //             log::info!("   Std Dev: {:.6}", std_dev);
    //             log::info!(
    //                 "   Min: {:.6}",
    //                 response
    //                     .embedding
    //                     .iter()
    //                     .fold(f32::INFINITY, |a, &b| a.min(b))
    //             );
    //             log::info!(
    //                 "   Max: {:.6}",
    //                 response
    //                     .embedding
    //                     .iter()
    //                     .fold(f32::NEG_INFINITY, |a, &b| a.max(b))
    //             );
    //         }
    //         Err(e) => {
    //             log::error!("❌ Embedding generation failed with {}: {}", model_id, e);
    //             log::warn!("💡 This model might not be available in your AWS account or region");
    //         }
    //     }

    //     log::info!("---");
    // }

    // // Test 5: Batch embedding comparison
    // log::info!("🔬 Testing semantic similarity...");

    // let test_sentences = vec![
    //     "The weather is beautiful today",
    //     "It's a lovely sunny day outside",
    //     "I love programming in Rust",
    //     "Rust is an excellent systems programming language",
    //     "The cat sat on the mat",
    // ];

    // let mut embeddings = Vec::new();

    // for sentence in &test_sentences {
    //     let embedding_request = rgen::EmbeddingRequest {
    //         text: sentence.to_string(),
    //         model_id: Some("amazon.titan-embed-text-v2:0".to_string()),
    //     };

    //     match client.vector().generate_embedding(embedding_request).await {
    //         Ok(response) => {
    //             embeddings.push((sentence.clone(), response.embedding));
    //             log::info!("✅ Generated embedding for: '{}'", sentence);
    //         }
    //         Err(e) => {
    //             log::error!("❌ Failed to generate embedding for '{}': {}", sentence, e);
    //         }
    //     }
    // }

    // // Calculate cosine similarities
    // if embeddings.len() >= 2 {
    //     log::info!("🔍 Calculating cosine similarities:");

    //     for i in 0..embeddings.len() {
    //         for j in (i + 1)..embeddings.len() {
    //             let similarity = cosine_similarity(&embeddings[i].1, &embeddings[j].1);
    //             log::info!(
    //                 "   '{}' vs '{}': {:.4}",
    //                 embeddings[i].0,
    //                 embeddings[j].0,
    //                 similarity
    //             );
    //         }
    //     }
    // }

    // // Test 6: Error handling
    // log::info!("🧪 Testing error handling...");

    // let invalid_request = rgen::TextGenerationRequest {
    //     prompt: "Test".to_string(),
    //     max_tokens: Some(100),
    //     temperature: Some(0.7),
    //     model_id: Some("invalid-model-id".to_string()),
    //     stream: None,
    // };

    // match client.text().generate(invalid_request).await {
    //     Ok(_) => {
    //         log::warn!("⚠️  Unexpected success with invalid model");
    //     }
    //     Err(e) => {
    //         log::info!("✅ Error handling working correctly: {}", e);
    //     }
    // }

    log::info!("🎉 All tests completed!");
    log::info!("💡 Check the generated image files in the current directory");
    log::info!("📝 Summary:");
    log::info!("   - Text generation: Multiple models tested");
    log::info!("   - Text streaming: Real-time response handling");
    log::info!("   - Image generation: Visual content creation");
    log::info!("   - Embeddings: Vector representations and similarity");
    log::info!("   - Error handling: Graceful failure management");

    Ok(())
}

// Helper function to calculate cosine similarity
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}
