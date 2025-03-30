use api_simulator::config::{AppConfig, ServerConfig, ProxyConfig};
use api_simulator::core::ApiSimulator;
use api_simulator::session::SessionMode;

use reqwest::Client;
use tokio;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use log::{debug, info, error, warn};

#[tokio::test]
async fn test_proxy_functionality() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging for the test
    let _ = env_logger::try_init();

    // Create a test config
    let config = AppConfig {
        server: ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 9090, // Use a different port for testing
            ..Default::default()
        },
        proxy: ProxyConfig {
            default_mode: SessionMode::Proxy,
            default_target: "".to_string(),
            forward_host_header: true,
        },
        ..Default::default()
    };

    // Start the server in a background task
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    let server_handle = tokio::spawn(async move {
        let simulator = match ApiSimulator::new(config).await {
            Ok(sim) => sim,
            Err(e) => {
                panic!("Failed to create simulator: {}", e);
            }
        };

        println!("Test server started on port 9090");

        match simulator.run().await {
            Ok(_) => println!("Server completed normally"),
            Err(e) => println!("Server error: {}", e),
        }
    });

    // Give the server time to start
    println!("Waiting for server to start...");
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create an HTTP client
    let client = Client::new();

    println!("Sending test request through proxy...");

    // Make a request through the proxy
    let response = client.get("http://127.0.0.1:9090/")
        .header("X-Proxy-Target", "http://httpbin.org/get")
        .timeout(Duration::from_secs(10))
        .send()
        .await?;

    // Check that we got a successful response
    println!("Received response with status: {}", response.status());
    assert!(response.status().is_success());

    let body = response.text().await?;
    println!("Response body: {}", body);

    // The response from httpbin.org/get should include the URL in the JSON
    assert!(body.contains("httpbin.org/get"));

    // Clean up - abort the server task since it's designed to run indefinitely
    server_handle.abort();
    println!("Test completed successfully!");

    Ok(())
}