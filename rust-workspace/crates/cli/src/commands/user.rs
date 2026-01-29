//! User commands

use anyhow::Result;
use clap::{Args, Subcommand};
use core::UserService;

#[derive(Args, Debug)]
pub struct UserArgs {
    #[command(subcommand)]
    pub action: UserAction,
}

#[derive(Subcommand, Debug)]
pub enum UserAction {
    /// Create a new user
    Create {
        /// User name
        #[arg(short, long)]
        name: String,
        
        /// User email
        #[arg(short, long)]
        email: String,
    },
    
    /// List all users
    List,
    
    /// Get user by ID
    Get {
        /// User ID
        #[arg(short, long)]
        id: u64,
    },
    
    /// Delete user by ID
    Delete {
        /// User ID
        #[arg(short, long)]
        id: u64,
    },
}

pub fn handle(args: UserArgs) -> Result<()> {
    let service = UserService::new();
    
    match args.action {
        UserAction::Create { name, email } => {
            // Validate email
            if !utils::validate_email(&email) {
                println!("❌ Invalid email format");
                return Ok(());
            }
            
            let user = service.create(&name, &email)?;
            println!("✅ Created user:");
            println!("{}", serde_json::to_string_pretty(&user)?);
        }
        
        UserAction::List => {
            let users = service.list();
            if users.is_empty() {
                println!("📭 No users found");
            } else {
                println!("👥 Users ({}):", users.len());
                for user in users {
                    println!("  - {} ({}): {}", user.id, user.name, user.email);
                }
            }
        }
        
        UserAction::Get { id } => {
            match service.get(id) {
                Ok(user) => {
                    println!("👤 User found:");
                    println!("{}", serde_json::to_string_pretty(&user)?);
                }
                Err(e) => println!("❌ {}", e),
            }
        }
        
        UserAction::Delete { id } => {
            match service.delete(id) {
                Ok(_) => println!("✅ User {} deleted", id),
                Err(e) => println!("❌ {}", e),
            }
        }
    }
    
    Ok(())
}
