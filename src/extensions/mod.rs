mod conflicts;
mod manager;
#[cfg(test)]
mod tests;

pub use manager::{ExtensionManager, InventoryExtension};

use self::manager::InventoryExtension as Extension;
use crate::models::common::{
    InventoryExtensionMetadata as Metadata, InventoryExtensionUniqueID as ExtensionID,
};
