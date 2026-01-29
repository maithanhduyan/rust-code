//! Config command

use anyhow::Result;
use utils::AppConfig;

pub fn handle() -> Result<()> {
    let config = AppConfig::from_env();
    
    println!("⚙️  Current Configuration:");
    println!("{}", serde_json::to_string_pretty(&config)?);
    println!();
    println!("📍 Bind address: {}", config.bind_address());
    
    Ok(())
}
