use reqwest;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Lectara API endpoints...");
    println!("Make sure the server is running with: cargo run");
    println!();

    let client = reqwest::Client::new();

    // Test health endpoint
    println!("Testing /health endpoint...");
    let response = client.get("http://localhost:3000/health").send().await?;

    println!("Status: {}", response.status());
    let text = response.text().await?;
    println!("Response: {}", text);
    println!();

    // Test content endpoint
    println!("Testing /content endpoint...");
    let dummy_content = json!({
        "url": "https://example.com/article",
        "title": "Test Article",
        "author": "Test Author"
    });

    let response = client
        .post("http://localhost:3000/content")
        .json(&dummy_content)
        .send()
        .await?;

    println!("Status: {}", response.status());
    let response_body: serde_json::Value = response.json().await?;
    println!("Response: {:#}", response_body);

    Ok(())
}
