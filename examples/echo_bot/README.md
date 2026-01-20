# Echo Bot Example

A simple demonstration of the Alloy bot framework using the handler system and runtime-managed bots.

## Features

- **Handler Pattern**: Shows how to implement handlers using the `Handler` trait
- **Custom Extractors**: Demonstrates the `FromContext` extractor pattern
- **Event Dispatching**: All messages flow through the dispatcher to handlers
- **Logging**: Comprehensive logging with `tracing`

## Commands

| Command | Description |
|---------|-------------|
| `/echo <text>` | Echoes the text back |
| `/ping` | Check if the bot is responsive |
| `/help` | Show available commands |
| `/info` | Show message info (extractor demo) |

## Configuration

Create an `alloy.yaml` file in the same directory as the executable:

```yaml
# Global settings
global:
  log_level: info
  timeout_ms: 30000
  retry:
    max_retries: 3
    initial_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0

# Bot instances
bots:
  - id: echo-bot
    name: Echo Bot
    adapter: onebot
    enabled: true
    transport:
      type: ws-client
      url: ws://127.0.0.1:8080/ws
      access_token: ${BOT_ACCESS_TOKEN:-}
      auto_reconnect: true
      heartbeat_interval_secs: 30
```

### Transport Types

The bot supports multiple transport types:

| Type | Description |
|------|-------------|
| `ws-client` | WebSocket client (connects to server) |
| `ws-server` | WebSocket server (accepts connections) |
| `http-client` | HTTP polling client |
| `http-server` | HTTP webhook receiver |

### Environment Variables

| Variable | Description |
|----------|-------------|
| `BOT_ACCESS_TOKEN` | Access token for authentication |
| `RUST_LOG` | Log level override (e.g., `debug`, `info`) |

## Running

```bash
# From the workspace root
cargo run --package echo-bot

# Or from the example directory
cd examples/echo_bot
cargo run
```

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     AlloyRuntime                        │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │   Adapter   │  │  Dispatcher  │  │   Registry    │  │
│  │  (OneBot)   │  │              │  │               │  │
│  └──────┬──────┘  └───────┬──────┘  └───────────────┘  │
│         │                 │                             │
│         │   BoxedEvent    │                             │
│         └────────────────>│                             │
│                           │                             │
│                   ┌───────┴───────┐                     │
│                   ▼               ▼                     │
│            LoggingHandler   EchoHandler                 │
│                   │               │                     │
│                   ▼               ▼                     │
│            PingHandler      HelpHandler                 │
└─────────────────────────────────────────────────────────┘
```

## Handler Examples

### Basic Handler

```rust
struct MyHandler;

impl Handler for MyHandler {
    fn check(&self, ctx: &AlloyContext) -> bool {
        // Return true if this handler should process the event
        ctx.event().is::<MyEventType>()
    }

    fn handle<'a>(&'a self, ctx: &'a AlloyContext) -> BoxFuture<'a, Outcome> {
        Box::pin(async move {
            // Handle the event
            Outcome::Handled
        })
    }
}
```

### With Custom Extractors

```rust
struct MessageText(String);

impl FromContext for MessageText {
    type Error = ();

    fn from_context(ctx: &AlloyContext) -> Result<Self, Self::Error> {
        if let Some(event) = ctx.event().downcast::<GroupMessage>() {
            return Ok(MessageText(event.text.clone()));
        }
        Err(())
    }
}

// Use in handler
fn check(&self, ctx: &AlloyContext) -> bool {
    if let Ok(MessageText(text)) = MessageText::from_context(ctx) {
        return text.starts_with("/mycommand");
    }
    false
}
```

## License

MPL-2.0
