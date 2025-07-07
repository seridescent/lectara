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
        /// Optional body text for the content
        #[arg(short, long)]
        body: Option<String>,
    },
}

#[derive(Serialize)]
struct NewContentItem {
    url: String,
    title: Option<String>,
    author: Option<String>,
    body: Option<String>,
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
        Commands::Add {
            url,
            title,
            author,
            body,
        } => {
            add_content(&client, &cli.service_url, url, title, author, body).await?;
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
    body: Option<String>,
) -> Result<(), Box<dyn Error>> {
    let endpoint = format!("{service_url}/api/v1/content");

    let payload = NewContentItem {
        url,
        title,
        author,
        body,
    };

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
