//! Simple IPFRS Server Example
//!
//! This example demonstrates the easiest way to start an IPFRS server
//! using the new configuration presets.
//!
//! # Usage
//!
//! Development mode (localhost, fast compression):
//! ```bash
//! cargo run --example simple_server -- dev
//! ```
//!
//! Production mode (all interfaces, maximum compression):
//! ```bash
//! cargo run --example simple_server -- prod
//! ```
//!
//! Testing mode (minimal overhead):
//! ```bash
//! cargo run --example simple_server -- test
//! ```

use ipfrs_interface::{Gateway, GatewayConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().init();

    // Get mode from command line args
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("dev");

    // Select configuration based on mode
    let config = match mode {
        "prod" | "production" => {
            println!("🚀 Starting in PRODUCTION mode");
            GatewayConfig::production()
        }
        "test" | "testing" => {
            println!("🧪 Starting in TESTING mode");
            GatewayConfig::testing()
        }
        _ => {
            println!("🔧 Starting in DEVELOPMENT mode");
            GatewayConfig::development()
        }
    };

    // Optional TLS from env (IPFRS_TLS_CERT / IPFRS_TLS_KEY) — needed for an
    // HTTPS console (GitHub Pages is https, so it cannot call an http gateway).
    let mut config = config;
    if let (Ok(cert), Ok(key)) = (std::env::var("IPFRS_TLS_CERT"), std::env::var("IPFRS_TLS_KEY")) {
        config.tls_config = Some(ipfrs_interface::tls::TlsConfig::new(cert, key));
        println!("🔒 TLS enabled from IPFRS_TLS_CERT / IPFRS_TLS_KEY");
    }

    // Validate configuration
    config.validate()?;

    println!("\n📋 Configuration:");
    println!("  Listen: {}", config.listen_addr);
    println!("  Storage: {}", config.storage_config.path.display());
    println!(
        "  Cache: {}MB",
        config.storage_config.cache_size / (1024 * 1024)
    );
    println!("\n");

    // Create and start gateway with GraphQL enabled
    let gateway = Gateway::new(config)?.with_knowledge().await?.with_graphql();

    println!("  Knowledge: /api/v0/knowledge/* enabled");
    gateway.start().await?;

    Ok(())
}
