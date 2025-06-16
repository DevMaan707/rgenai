# rgenai - Rust AWS Bedrock Client Library

[![Crates.io](https://img.shields.io/crates/v/rgenai.svg)](https://crates.io/crates/rgenai)
[![Documentation](https://docs.rs/rgenai/badge.svg)](https://docs.rs/rgenai)
[![License](https://img.shields.io/crates/l/rgenai.svg)](LICENSE)

**RGenAi** is a Rust library for AWS Bedrock that provides easy-to-use clients for text generation, image generation, embeddings, and vector storage. Built with performance, type safety, and developer experience in mind.

## 🚀 Features

### 🤖 AI Model Support
- **Text Generation**: Support for multiple models including:
  - Amazon Titan
  - Anthropic Claude (with Claude 4 support with profile inference)
  - Meta Llama
  - Mistral
  - AI21
  - Cohere
- **Image Generation**: Amazon Titan Image Generator
- **Embeddings**: Amazon Titan Embed model
- **Streaming**: Real-time text generation with async streams

### 🗄️ Vector Storage Backend
- **PostgreSQL** with pgvector extension (Primary supported backend)
- **Note**: Pinecone and Upstash integrations are planned but not currently functional

### 🔧 Advanced Features
- **RAG (Retrieval Augmented Generation)**: Built-in context retrieval and generation
- **Semantic Search**: Vector similarity search with PostgreSQL/pgvector
- **Beautiful Logging**: Structured logging with emojis and colors
- **Profile Inference**: Automatic model profile selection
- **Configuration Management**: Environment-based configuration

## 📦 Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
rgenai = "0.1.0"

# For PostgreSQL vector storage support
rgenai = { version = "0.1.0", features = ["postgres"] }
```

### Feature Flags

- `postgres` - Enable PostgreSQL with pgvector support (Recommended)
- `pinecone` - Pinecone support (Coming soon)
- `upstash` - Upstash support (Coming soon)

## 🏁 Quick Start

### Basic Text Generation

```rust
use rgenai::{BedrockClient, BedrockConfig, TextGenerationRequest};

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
        prompt: "create me a very well designed frontend!".to_string(),
        max_tokens: Some(150),
        temperature: Some(0.7),
        model_id: Some(
            "arn:aws:bedrock:us-east-1:022499029734:application-inference-profile/....... {REPLACE WHOLE MODEL ID WITH YOUR INFERENCE ID}"
                .to_string(),
        ),
        stream: None,
        provider: Some(ModelProvider::Anthropic)
    };

    let response = client.text().generate(request).await?;
    println!("{:?}", response);

    Ok(())
}
```

### Vector Storage with PostgreSQL

```rust
use rgenai::{BedrockClient, BedrockConfig, Config, PostgresConfig};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bedrock_config = BedrockConfig::new();
    let storage_config = Config::new()
        .with_postgres(
            PostgresConfig::new()
                .with_connection_info("localhost", 5432, "vectordb")
                .with_credentials("user", "password")
        );

    let client = BedrockClient::with_storage(bedrock_config, storage_config).await?;
    let result = client.embed_and_store(
        "Rust is a systems programming language focused on safety and performance.",
        None,
        Some(HashMap::new()),
        Some("docs"),
    ).await?;

    println!("Stored document with ID: {}", result.id);
    let answer = client.generate_with_context(
        "What is Rust programming language?",
        3,
        None,
        None,
        Some("docs"),
        Some(200),
        Some(0.7),
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

# PostgreSQL Configuration (Required for vector storage)
USE_PSQL=true
POSTGRES_HOST=localhost
POSTGRES_PORT=5432
POSTGRES_USERNAME=postgres
POSTGRES_PASSWORD=password
POSTGRES_DATABASE=vectordb

# Application Configuration
PORT=8080
```

### PostgreSQL Setup

1. Install PostgreSQL and pgvector extension
2. Create database and enable extension:

```sql
CREATE DATABASE vectordb;
\c vectordb
CREATE EXTENSION vector;
```

3. The library will automatically create required tables and indexes

## 🤖 Default Models

When no model_id is provided, the library uses these defaults:

- Text Generation: `amazon.titan-text-express-v1`
- Image Generation: `amazon.titan-image-generator-v1`
- Embeddings: `amazon.titan-embed-text-v1`

## 📝 Logging

```rust
use rgenai::logger::{init_with_config, LoggerConfig, LogLevel};

let logger_config = LoggerConfig::new()
    .with_level(LogLevel::Debug)
    .with_colors(true)
    .with_file_output("app.log");

init_with_config(logger_config)?;
```

## 🛠️ Error Handling

```rust
use rgenai::{BedrockError, Result};

match client.text().generate(request).await {
    Ok(response) => println!("Success: {}", response),
    Err(BedrockError::ConfigError(msg)) => eprintln!("Configuration error: {}", msg),
    Err(BedrockError::AwsError(msg)) => eprintln!("AWS error: {}", msg),
    Err(e) => eprintln!("Other error: {}", e),
}
```

## 🔍 Known Limitations

1. Currently, only PostgreSQL with pgvector is fully supported for vector storage
2. Pinecone and Upstash integrations are in development
3. Some advanced model parameters may not be exposed yet

## 🤝 Contributing

Contributions are welcome! Priority areas:

1. Implementing Pinecone and Upstash storage backends
2. Adding more model parameter controls
3. Improving documentation and examples
4. Adding tests and benchmarks

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 📞 Support

- **Documentation**: [docs.rs/rgenai](https://docs.rs/rgenai)
- **Issues**: [GitHub Issues](https://github.com/DevMaan707/rgenai/issues)
- **Discussions**: [GitHub Discussions](https://github.com/DevMaan707/rgenai/discussions)

---

Built with ❤️ by DevMaan707 - Empowering Rust AI applications 🦀
