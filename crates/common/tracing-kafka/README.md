# tracing-kafka

A Kafka layer for the `tracing` ecosystem that sends structured logs to Apache Kafka.

## Features

- **Seamless Integration**: Works as a standard `tracing` layer
- **Structured JSON Output**: Consistent JSON formatting for all log events
- **Non-blocking**: Kafka failures won't block your application
- **Configurable Batching**: Control message batching and flushing behavior
- **Custom Fields**: Add static fields to all log entries

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tracing-kafka = { path = "../tracing-kafka" }
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Quick Start

```rust
use tracing_kafka::KafkaLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a Kafka layer
    let kafka_layer = KafkaLayer::builder()
        .brokers(&["localhost:9092"])
        .topic("application-logs")
        .build()?;

    // Initialize tracing with the Kafka layer
    tracing_subscriber::registry()
        .with(kafka_layer)
        .init();

    // Your logs are now sent to Kafka
    tracing::info!("Application started");
    tracing::error!(error = "Connection failed", "Database error");

    Ok(())
}
```

## Configuration Options

### Basic Configuration

```rust
use std::time::Duration;

let kafka_layer = KafkaLayer::builder()
    .brokers(&["broker1:9092", "broker2:9092"])
    .topic("my-app-logs")
    .batch_size(1000)                        // Batch up to 1000 messages
    .flush_interval(Duration::from_secs(5))  // Flush every 5 seconds
    .build()?;
```

### Adding Static Fields

Add fields that appear in every log entry:

```rust
use std::collections::HashMap;
use serde_json::json;

let static_fields = HashMap::from([
    ("service".to_string(), json!("my-service")),
    ("version".to_string(), json!("1.0.0")),
    ("environment".to_string(), json!("production")),
]);

let kafka_layer = KafkaLayer::with_config(
    vec!["localhost:9092".to_string()],
    "application-logs".to_string(),
    static_fields,
)?;
```

## Output Format

Logs are sent to Kafka as JSON:

```json
{
  "message": "User authentication successful",
  "level": "INFO",
  "timestamp": "2025-01-07T10:30:00.123Z",
  "target": "my_app::auth",
  "file": "src/auth.rs",
  "line": 42,
  "hostname": "server-01",
  "pid": 1234,
  "service": "my-service",
  "version": "1.0.0"
}
```

## Batching Behavior

The layer uses Kafka's producer batching to optimize throughput:

- **batch_size**: Maximum number of bytes to batch before sending (default: 16KB)
- **flush_interval**: Maximum time to wait before sending a batch (default: 0ms)

Messages are sent when either condition is met first.

### Examples:

```rust
// Real-time logging (default)
KafkaLayer::builder()
    .brokers(&["localhost:9092"])
    .topic("logs")
    .build()?;

// Optimized for throughput
KafkaLayer::builder()
    .brokers(&["localhost:9092"])
    .topic("logs")
    .batch_size(65536)  // 64KB batches
    .flush_interval(Duration::from_millis(100))
    .build()?;
```

## Error Handling

The layer is designed to be resilient:

- Kafka connection failures are logged but don't crash the application
- Failed messages are dropped after retry attempts
- The application continues running even if Kafka is unavailable

## Performance Considerations

- **Async Operations**: All Kafka operations are non-blocking
- **Buffering**: Messages are buffered internally by the Kafka producer
- **Memory Usage**: Large batch sizes increase memory usage
- **Latency vs Throughput**: Adjust `flush_interval` based on your needs


## Requirements

- Rust 1.70 or later
- Apache Kafka 0.11 or later

## License

This project is licensed under the same terms as the parent workspace.
