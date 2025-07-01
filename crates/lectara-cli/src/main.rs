use clap::{Parser, Subcommand};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Parser)]
#[command(name = "lectara")]
#[command(about = "A CLI for managing content collection")]
struct Cli {
    /// Base URL for the Lectara service
    #[arg(long, default_value = "http://localhost:3000")]
    service_url: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Add content to the collection
    Add {
        /// URL of the content to add
        url: String,
        /// Optional title for the content
        #[arg(short, long)]
        title: Option<String>,
        /// Optional author of the content
        #[arg(short, long)]
        author: Option<String>,
    },
    /// Check service health
    Health,
}

#[derive(Serialize)]
struct NewContentItem {
    url: String,
    title: Option<String>,
    author: Option<String>,
}

#[derive(Deserialize)]
struct ContentResponse {
    id: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    let client = Client::new();

    match cli.command {
        Commands::Add { url, title, author } => {
            add_content(&client, &cli.service_url, url, title, author).await?;
        }
        Commands::Health => {
            check_health(&client, &cli.service_url).await?;
        }
    }

    Ok(())
}

async fn add_content(
    client: &Client,
    service_url: &str,
    url: String,
    title: Option<String>,
    author: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let endpoint = format!("{service_url}/content");

    let payload = NewContentItem { url, title, author };

    let response = client.post(&endpoint).json(&payload).send().await?;

    if response.status().is_success() {
        let content_response: ContentResponse = response.json().await?;
        println!(
            "Content added successfully with ID: {}",
            content_response.id
        );
    } else {
        eprintln!("Failed to add content: {}", response.status());
        eprintln!("Response: {}", response.text().await?);
    }

    Ok(())
}

async fn check_health(client: &Client, service_url: &str) -> Result<(), Box<dyn Error>> {
    let endpoint = format!("{service_url}/health");

    let response = client.get(&endpoint).send().await?;

    if response.status().is_success() {
        let health_status = response.text().await?;
        println!("Service health: {health_status}");
    } else {
        eprintln!("Health check failed: {}", response.status());
    }

    Ok(())
}
