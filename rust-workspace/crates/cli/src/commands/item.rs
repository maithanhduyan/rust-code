//! Item commands

use anyhow::Result;
use clap::{Args, Subcommand};
use core::ItemService;

#[derive(Args, Debug)]
pub struct ItemArgs {
    #[command(subcommand)]
    pub action: ItemAction,
}

#[derive(Subcommand, Debug)]
pub enum ItemAction {
    /// Create a new item
    Create {
        /// Item title
        #[arg(short, long)]
        title: String,
        
        /// Owner user ID
        #[arg(short, long)]
        owner: u64,
        
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    
    /// List items by owner
    List {
        /// Owner user ID
        #[arg(short, long)]
        owner: u64,
    },
    
    /// Get item by ID
    Get {
        /// Item ID
        #[arg(short, long)]
        id: u64,
    },
}

pub fn handle(args: ItemArgs) -> Result<()> {
    let service = ItemService::new();
    
    match args.action {
        ItemAction::Create { title, owner, description } => {
            let mut item = service.create(&title, owner)?;
            if let Some(desc) = description {
                item.description = Some(desc);
            }
            println!("✅ Created item:");
            println!("{}", serde_json::to_string_pretty(&item)?);
        }
        
        ItemAction::List { owner } => {
            let items = service.list_by_owner(owner);
            if items.is_empty() {
                println!("📭 No items found for owner {}", owner);
            } else {
                println!("📦 Items for owner {} ({}):", owner, items.len());
                for item in items {
                    let desc = item.description.unwrap_or_else(|| "No description".to_string());
                    println!("  - [{}] {}: {}", item.id, item.title, utils::truncate(&desc, 30, "..."));
                }
            }
        }
        
        ItemAction::Get { id } => {
            match service.get(id) {
                Ok(item) => {
                    println!("📦 Item found:");
                    println!("{}", serde_json::to_string_pretty(&item)?);
                }
                Err(e) => println!("❌ {}", e),
            }
        }
    }
    
    Ok(())
}
