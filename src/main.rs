mod database;
mod extension;
mod models;

use database::Database;
use extension::ExtensionManager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let db = Database::connect().await;
    db.setup_tables().await?;
    db.setup_reserved_items().await?;

    let manager = ExtensionManager::new()?;
    manager.load_extensions(&db).await?;

    Ok(())
}
