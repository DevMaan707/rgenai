# rgenai - Rust AWS Bedrock Client Library

[![Crates.io](https://img.shields.io/crates/v/rgenai.svg)](https://crates.io/crates/rgenai)
[![Documentation](https://docs.rs/rgenai/badge.svg)](https://docs.rs/rgenai)
[![License](https://img.shields.io/crates/l/rgenai.svg)](LICENSE)
[![Build Status](https://github.com/DevMaan707/rgenai/workflows/CI/badge.svg)](https://github.com/DevMaan707/rgenai/actions)

**RGenAi** is a comprehensive Rust library for AWS Bedrock that provides easy-to-use clients for text generation, image generation, embeddings, and vector storage. Built with performance, type safety, and developer experience in mind.

## 🚀 Features

### 🤖 AI Model Support
- **Text Generation**: Support for 20+ models including Amazon Titan, Anthropic Claude, Meta Llama, Mistral, AI21, and Cohere
- **Image Generation**: Amazon Titan Image Generator and Stability AI Stable Diffusion XL
- **Embeddings**: Amazon Titan Embed and Cohere Embed models
- **Streaming**: Real-time text generation with async streams

### 🗄️ Vector Storage Backends
- **PostgreSQL** with pgvector extension
- **Pinecone** vector database
- **Upstash** vector service
- Unified storage interface with automatic client management

### 🔧 Advanced Features
- **RAG (Retrieval Augmented Generation)**: Built-in context retrieval and generation
- **Semantic Search**: Vector similarity search across storage backends
- **Batch Operations**: Efficient bulk insert/update/delete operations
- **Beautiful Logging**: Structured logging with emojis and colors
- **Configuration Management**: Environment-based configuration with validation

### ⚡ Performance & Reliability
- Async/await throughout with Tokio
- Connection pooling for database backends
- Comprehensive error handling and recovery
- Type-safe API with Rust's type system
- Memory efficient streaming for large responses

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rgenai = "0.1.0"

# Optional features
rgenai = { version = "0.1.0", features = ["postgres", "pinecone", "upstash"] }
```

### Feature Flags

- `postgres` - Enable PostgreSQL with pgvector support
- `pinecone` - Enable Pinecone vector database support
- `upstash` - Enable Upstash vector service support

## 🏁 Quick Start

### Basic Text Generation

```rust
use rgenai::{BedrockClient, BedrockConfig, TextGenerationRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    rgenai::logger::init()?;

    // Create Bedrock client
    let config = BedrockConfig::new()
        .with_region("us-east-1");

    let client = BedrockClient::new(config).await?;

    // Generate text
    let request = TextGenerationRequest {
        prompt: "Write a haiku about Rust programming".to_string(),
        max_tokens: Some(100),
        temperature: Some(0.7),
        model_id: Some("anthropic.claude-3-haiku-20240307-v1:0".to_string()),
        stream: None,
    };

    let response = client.text().generate(request).await?;
    println!("Generated text: {}", response.text);

    Ok(())
}
```

### Streaming Text Generation

```rust
use futures::StreamExt;
use rgenai::{BedrockClient, BedrockConfig, TextGenerationRequest};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BedrockConfig::new();
    let client = BedrockClient::new(config).await?;

    let request = TextGenerationRequest {
        prompt: "Tell me a story about AI".to_string(),
        max_tokens: Some(200),
        temperature: Some(0.8),
        model_id: Some("amazon.titan-text-express-v1".to_string()),
        stream: Some(true),
    };

    let mut stream = client.text().generate_stream(request).await?;

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) => {
                print!("{}", chunk.chunk);
                if chunk.done {
                    break;
                }
            }
            Err(e) => eprintln!("Stream error: {}", e),
        }
    }

    Ok(())
}
```

### Image Generation

```rust
use rgenai::{BedrockClient, BedrockConfig, ImageGenerationRequest};
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = BedrockConfig::new();
    let client = BedrockClient::new(config).await?;

    let request = ImageGenerationRequest {
        prompt: "A serene landscape with mountains at sunset, digital art".to_string(),
        model_id: Some("amazon.titan-image-generator-v1".to_string()),
        width: Some(1024),
        height: Some(1024),
        num_images: Some(1),
    };

    let response = client.image().generate(request).await?;

    // Save image
    let image_bytes = base64::decode(&response.image_data)?;
    fs::write("generated_image.png", image_bytes)?;

    println!("Image saved to generated_image.png");
    Ok(())
}
```

### Vector Storage and RAG

```rust
use rgenai::{BedrockClient, BedrockConfig, Config, PostgresConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure with storage backend
    let bedrock_config = BedrockConfig::new();
    let storage_config = Config::new()
        .with_postgres(
            PostgresConfig::new()
                .with_connection_info("localhost", 5432, "vectordb")
                .with_credentials("user", "password")
        );

    let client = BedrockClient::with_storage(bedrock_config, storage_config).await?;

    // Store documents with embeddings
    let documents = vec![
        "Rust is a systems programming language focused on safety and performance.",
        "Machine learning models require large amounts of training data.",
        "Vector databases enable semantic search and retrieval augmented generation.",
    ];

    for (i, doc) in documents.iter().enumerate() {
        let mut metadata = HashMap::new();
        metadata.insert("doc_id".to_string(), serde_json::json!(i));
        metadata.insert("category".to_string(), serde_json::json!("tech"));

        let result = client.embed_and_store(
            doc,
            Some("amazon.titan-embed-text-v2:0"),
            Some(metadata),
            Some("docs"),
        ).await?;

        println!("Stored document {} with ID: {}", i, result.id);
    }

    // Perform RAG query
    let answer = client.generate_with_context(
        "What is Rust programming language?",
        3, // context limit
        Some("anthropic.claude-3-haiku-20240307-v1:0"), // generation model
        Some("amazon.titan-embed-text-v2:0"), // embedding model
        Some("docs"), // namespace
        Some(200), // max tokens
        Some(0.7), // temperature
    ).await?;

    println!("RAG Answer: {}", answer);

    Ok(())
}
```

## 🎛️ Configuration

### Environment Variables

```bash
# AWS Configuration
AWS_REGION=us-east-1
AWS_ACCESS_KEY_ID=your_access_key
AWS_SECRET_ACCESS_KEY=your_secret_key

# PostgreSQL Configuration
USE_PSQL=true
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_USERNAME=postgres
POSTGRES_PASSWORD=password
POSTGRES_DATABASE=vectordb

# Pinecone Configuration
USE_PINECONE=true
PINECONE_API_KEY=your_api_key
PINECONE_ENVIRONMENT=us-east-1-aws
PINECONE_INDEX_NAME=your_index

# Upstash Configuration
USE_UPSTASH=true
UPSTASH_URL=https://your-endpoint.upstash.io
UPSTASH_TOKEN=your_token

# Application Configuration
PORT=8080
```

### Programmatic Configuration

```rust
use rgenai::{BedrockConfig, Config, PostgresConfig, PineconeConfig};

let bedrock_config = BedrockConfig::new()
    .with_region("us-west-2")
    .with_credentials("access_key", "secret_key");

let storage_config = Config::new()
    .with_port(3000)
    .with_postgres(
        PostgresConfig::new()
            .with_connection_info("localhost", 5432, "mydb")
            .with_credentials("user", "pass")
    )
    .with_pinecone(
        PineconeConfig::new()
            .with_credentials("api_key")
            .with_environment("us-east-1-aws")
            .with_index("my-index")
    );
```

## 🤖 Supported Models

### Text Generation Models

| Provider | Model ID | Model Name |
|----------|----------|------------|
| Amazon | `amazon.titan-text-express-v1` | Titan Text Express |
| Amazon | `amazon.titan-text-lite-v1` | Titan Text Lite |
| Amazon | `amazon.titan-text-premier-v1:0` | Titan Text Premier |
| Anthropic | `anthropic.claude-3-5-sonnet-20241022-v2:0` | Claude 3.5 Sonnet |
| Anthropic | `anthropic.claude-3-sonnet-20240229-v1:0` | Claude 3 Sonnet |
| Anthropic | `anthropic.claude-3-haiku-20240307-v1:0` | Claude 3 Haiku |
| Anthropic | `anthropic.claude-3-opus-20240229-v1:0` | Claude 3 Opus |
| Meta | `meta.llama3-8b-instruct-v1:0` | Llama 3 8B Instruct |
| Meta | `meta.llama3-70b-instruct-v1:0` | Llama 3 70B Instruct |
| Meta | `meta.llama3-1-405b-instruct-v1:0` | Llama 3.1 405B Instruct |
| Mistral | `mistral.mistral-7b-instruct-v0:2` | Mistral 7B Instruct |
| Mistral | `mistral.mixtral-8x7b-instruct-v0:1` | Mixtral 8x7B Instruct |
| AI21 | `ai21.jamba-instruct-v1:0` | Jamba Instruct |
| Cohere | `cohere.command-r-plus-v1:0` | Command R+ |

### Image Generation Models

| Provider | Model ID | Model Name |
|----------|----------|------------|
| Amazon | `amazon.titan-image-generator-v1` | Titan Image Generator |
| Amazon | `amazon.titan-image-generator-v2:0` | Titan Image Generator V2 |
| Stability AI | `stability.stable-diffusion-xl-v1:0` | Stable Diffusion XL |

### Embedding Models

| Provider | Model ID | Model Name |
|----------|----------|------------|
| Amazon | `amazon.titan-embed-text-v2:0` | Titan Text Embeddings V2 |
| Cohere | `cohere.embed-english-v3` | Cohere English Embeddings |

## 📊 Vector Storage

### PostgreSQL with pgvector

```rust
use rgenai::{Config, PostgresConfig, VectorStorageManager};

let config = Config::new()
    .with_postgres(
        PostgresConfig::new()
            .with_connection_info("localhost", 5432, "vectordb")
            .with_credentials("postgres", "password")
    );

let storage = VectorStorageManager::new(config).await?;

// Insert vectors
let insert = VectorInsert {
    id: None,
    vector: vec![0.1, 0.2, 0.3, /* ... */],
    metadata: HashMap::new(),
    content: Some("Sample document content".to_string()),
    namespace: Some("docs".to_string()),
};

let result = storage.insert(insert).await?;
```

### Pinecone

```rust
use rgenai::{Config, PineconeConfig, VectorStorageManager};

let config = Config::new()
    .with_pinecone(
        PineconeConfig::new()
            .with_credentials("your-api-key")
            .with_environment("us-east-1-aws")
            .with_index("your-index")
    );

let storage = VectorStorageManager::new(config).await?;
```

### Upstash

```rust
use rgenai::{Config, UpstashConfig, VectorStorageManager};

let config = Config::new()
    .with_upstash(
        UpstashConfig::new()
            .with_credentials("https://endpoint.upstash.io", "your-token")
    );

let storage = VectorStorageManager::new(config).await?;
```

## 🔍 Semantic Search

```rust
use rgenai::VectorSearch;

let search_query = VectorSearch {
    vector: query_embedding,
    limit: 10,
    namespace: Some("documents".to_string()),
    filter: None,
    include_metadata: true,
    include_content: true,
};

let results = storage.search(search_query).await?;

for result in results.results {
    println!("ID: {}, Score: {:.4}, Content: {:?}",
        result.id, result.score, result.content);
}
```

## 📝 Logging

rgenai includes a beautiful, structured logging system:

```rust
use rgenai::logger::{init_with_config, LoggerConfig, LogLevel};

// Initialize with custom configuration
let logger_config = LoggerConfig::new()
    .with_level(LogLevel::Debug)
    .with_colors(true)
    .with_file_output("app.log");

init_with_config(logger_config)?;

// Use throughout your application
log::info!("Application started");
log::debug!("Debug information");
log::error!("Something went wrong");
```

## 🛠️ Error Handling

rgenai provides comprehensive error handling:

```rust
use rgenai::{BedrockError, Result};

match client.text().generate(request).await {
    Ok(response) => println!("Success: {}", response.text),
    Err(BedrockError::ConfigError(msg)) => eprintln!("Configuration error: {}", msg),
    Err(BedrockError::RequestError(msg)) => eprintln!("Request error: {}", msg),
    Err(BedrockError::ResponseError(msg)) => eprintln!("Response error: {}", msg),
    Err(BedrockError::AwsError(msg)) => eprintln!("AWS error: {}", msg),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## 📚 Examples

Check out the `examples/` directory for complete examples:

- `basic_text_generation.rs` - Simple text generation
- `streaming_chat.rs` - Real-time streaming responses
- `image_generation.rs` - Generate and save images
- `rag_system.rs` - Complete RAG implementation
- `vector_operations.rs` - Vector storage operations
- `multi_model_comparison.rs` - Compare responses across models

## 🏗️ Architecture

```
rgenai/
├── src/
│   ├── bedrock/          # AWS Bedrock clients
│   │   ├── text_client.rs
│   │   ├── image_client.rs
│   │   └── vector_client.rs
│   ├── storage/          # Vector storage backends
│   │   ├── postgres.rs
│   │   ├── pinecone.rs
│   │   └── upstash.rs
│   ├── models/           # Data structures
│   ├── config/           # Configuration management
│   ├── logger/           # Logging system
│   └── error/            # Error handling
```

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request. For major changes, please open an issue first to discuss what you would like to change.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/DevMaan707/rgenai.git
cd rgenai

# Install dependencies
cargo build

# Run tests
cargo test

# Run examples
cargo run --example basic_text_generation
```

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- AWS Bedrock team for the excellent AI model APIs
- The Rust community for amazing crates and tools
- Contributors to pgvector, Pinecone, and Upstash for vector storage solutions

## 📞 Support

- **Documentation**: [docs.rs/rgenai](https://docs.rs/rgenai)
- **Issues**: [GitHub Issues](https://github.com/DevMaan707/rgenai/issues)
- **Discussions**: [GitHub Discussions](https://github.com/DevMaan707/rgenaiai/discussions)

---

**Created by DevMaan707** - Building the future of AI applications in Rust 🦀
