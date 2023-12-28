use std::fmt::Debug;
use std::hash::Hash;

use crate::database::{
    DEVICE_CATEGORY_TABLE_NAME, DEVICE_MANUFACTURER_TABLE_NAME, DEVICE_TABLE_NAME,
    EXTENSION_TABLE_NAME,
};

/// A trait for ID types which are used as "primary keys" (unique string identifiers) in the
/// database, as opposed to Surreal's auto-generated UUIDs (used for non-unique items).
pub trait UniqueID: Debug + Clone + PartialEq + Eq + Hash + PartialOrd + Ord {
    const TABLE_NAME: &'static str;
    fn new(id: impl Into<String>) -> Self;
    fn unnamespaced(&self) -> &str;
    fn namespaced(&self) -> String {
        [Self::TABLE_NAME, &self.unnamespaced()].join(":")
    }
}

/// An unnamespaced unique extension ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InventoryExtensionUniqueID(String);

/// An unnamespaced unique device manufacturer ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceManufacturerUniqueID(String);

/// An unnamespaced unique device category ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceCategoryUniqueID(String);

/// An unnamespaced unique device ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceUniqueID(String);

impl UniqueID for InventoryExtensionUniqueID {
    const TABLE_NAME: &'static str = EXTENSION_TABLE_NAME;
    fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    fn unnamespaced(&self) -> &str {
        &self.0
    }
}

impl UniqueID for DeviceManufacturerUniqueID {
    const TABLE_NAME: &'static str = DEVICE_MANUFACTURER_TABLE_NAME;
    fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    fn unnamespaced(&self) -> &str {
        &self.0
    }
}

impl UniqueID for DeviceCategoryUniqueID {
    const TABLE_NAME: &'static str = DEVICE_CATEGORY_TABLE_NAME;
    fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    fn unnamespaced(&self) -> &str {
        &self.0
    }
}

impl UniqueID for DeviceUniqueID {
    const TABLE_NAME: &'static str = DEVICE_TABLE_NAME;
    fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    fn unnamespaced(&self) -> &str {
        &self.0
    }
}
