//! Configuration validation utilities.

use super::error::{ConfigError, ConfigResult};
use super::schema::{AlloyConfig, BotConfig, TransportConfig};
use std::collections::HashSet;

/// Validates the entire configuration.
pub fn validate_config(config: &AlloyConfig) -> ConfigResult<()> {
    validate_global_config(config)?;
    validate_bots_config(&config.bots)?;
    Ok(())
}

/// Validates global configuration settings.
fn validate_global_config(config: &AlloyConfig) -> ConfigResult<()> {
    // Validate log level
    let valid_log_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_log_levels.contains(&config.global.log_level.to_lowercase().as_str()) {
        return Err(ConfigError::validation(format!(
            "Invalid log level: {}. Valid values are: {:?}",
            config.global.log_level, valid_log_levels
        )));
    }

    // Validate timeout
    if config.global.timeout_ms == 0 {
        return Err(ConfigError::validation("Timeout must be greater than 0"));
    }

    // Validate retry config
    validate_retry_config(&config.global.retry)?;

    Ok(())
}

/// Validates retry configuration.
fn validate_retry_config(retry: &super::schema::RetryConfig) -> ConfigResult<()> {
    if retry.initial_delay_ms == 0 {
        return Err(ConfigError::validation(
            "Initial retry delay must be greater than 0",
        ));
    }

    if retry.max_delay_ms < retry.initial_delay_ms {
        return Err(ConfigError::validation(
            "Max retry delay must be greater than or equal to initial delay",
        ));
    }

    if retry.backoff_multiplier < 1.0 {
        return Err(ConfigError::validation(
            "Backoff multiplier must be at least 1.0",
        ));
    }

    Ok(())
}

/// Validates all bot configurations.
fn validate_bots_config(bots: &[BotConfig]) -> ConfigResult<()> {
    let mut seen_ids = HashSet::new();

    for bot in bots {
        // Check for duplicate IDs
        if !seen_ids.insert(&bot.id) {
            return Err(ConfigError::DuplicateBotId(bot.id.clone()));
        }

        validate_bot_config(bot)?;
    }

    Ok(())
}

/// Validates a single bot configuration.
fn validate_bot_config(bot: &BotConfig) -> ConfigResult<()> {
    // Validate bot ID
    if bot.id.is_empty() {
        return Err(ConfigError::missing_field("bot.id"));
    }

    if bot.id.contains(' ') {
        return Err(ConfigError::validation("Bot ID cannot contain spaces"));
    }

    // Validate adapter
    if bot.adapter.is_empty() {
        return Err(ConfigError::missing_field("bot.adapter"));
    }

    // Validate transport
    validate_transport_config(&bot.transport)?;

    Ok(())
}

/// Validates transport configuration.
fn validate_transport_config(transport: &TransportConfig) -> ConfigResult<()> {
    match transport {
        TransportConfig::WsClient(config) => {
            validate_url(&config.url, "ws")?;
            if let Some(ref retry) = config.retry {
                validate_retry_config(retry)?;
            }
        }
        TransportConfig::WsServer(config) => {
            validate_port(config.port)?;
            validate_path(&config.path)?;
        }
        TransportConfig::HttpClient(config) => {
            validate_url(&config.url, "http")?;
            if let Some(ref retry) = config.retry {
                validate_retry_config(retry)?;
            }
        }
        TransportConfig::HttpServer(config) => {
            validate_port(config.port)?;
            validate_path(&config.path)?;
        }
    }

    Ok(())
}

/// Validates a URL.
fn validate_url(url: &str, expected_scheme: &str) -> ConfigResult<()> {
    if url.is_empty() {
        return Err(ConfigError::missing_field("url"));
    }

    // Basic URL validation
    let valid_schemes = match expected_scheme {
        "ws" => ["ws://", "wss://"],
        "http" => ["http://", "https://"],
        _ => return Err(ConfigError::validation("Unknown URL scheme type")),
    };

    if !valid_schemes.iter().any(|s| url.starts_with(s)) {
        return Err(ConfigError::invalid_url(
            url,
            format!("URL must start with one of: {:?}", valid_schemes),
        ));
    }

    Ok(())
}

/// Validates a port number.
fn validate_port(port: u16) -> ConfigResult<()> {
    if port == 0 {
        return Err(ConfigError::InvalidPort(port));
    }
    Ok(())
}

/// Validates a path.
fn validate_path(path: &str) -> ConfigResult<()> {
    if !path.starts_with('/') {
        return Err(ConfigError::validation("Path must start with '/'"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_config() {
        let config = AlloyConfig::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_log_level() {
        let mut config = AlloyConfig::default();
        config.global.log_level = "invalid".to_string();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_duplicate_bot_id() {
        let bot = BotConfig {
            id: "test-bot".to_string(),
            name: None,
            adapter: "onebot".to_string(),
            transport: TransportConfig::WsClient(super::super::schema::WsClientConfig {
                url: "ws://localhost:8080".to_string(),
                access_token: None,
                auto_reconnect: true,
                heartbeat_interval_secs: 30,
                retry: None,
            }),
            enabled: true,
            settings: Default::default(),
        };

        let config = AlloyConfig {
            global: Default::default(),
            bots: vec![bot.clone(), bot],
        };

        let result = validate_config(&config);
        assert!(matches!(result, Err(ConfigError::DuplicateBotId(_))));
    }
}
