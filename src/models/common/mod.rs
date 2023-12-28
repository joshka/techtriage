mod ids;

pub use ids::{
    DeviceCategoryUniqueID, DeviceManufacturerUniqueID, InventoryExtensionUniqueID, UniqueID,
};

use std::collections::HashSet;

use semver::Version;

/// The metadata of an inventory extension.
/// This does not include the extension contents, such as devices or manufacturers.
/// Used to identify existing extensions to the
/// [`ExtensionManager`](crate::extensions::ExtensionManager) to prevent conflicts.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryExtensionMetadata {
    pub id: InventoryExtensionUniqueID,
    pub display_name: String,
    pub version: Version,
}

/// A device manufacturer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceManufacturer {
    pub id: DeviceManufacturerUniqueID,
    pub display_name: String,
    pub extensions: HashSet<InventoryExtensionUniqueID>,
}

/// A category of device, such as a phone, tablet, or gaming console.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceCategory {
    pub id: DeviceCategoryUniqueID,
    pub display_name: String,
    pub extensions: HashSet<InventoryExtensionUniqueID>,
}

/// A device and all of its relevant metadata, such as its make and model.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub internal_id: String,
    pub display_name: String,
    pub manufacturer: DeviceManufacturerUniqueID,
    pub category: DeviceCategoryUniqueID,
    pub extension: InventoryExtensionUniqueID,
    pub primary_model_identifiers: Vec<String>,
    pub extended_model_identifiers: Vec<String>,
}

impl DeviceManufacturer {
    /// Merges the extensions field of another device manufacturer into this one.
    /// Does not check whether the two device manufacturers share the same ID and other metadata.
    pub fn merge(&mut self, other: DeviceManufacturer) {
        self.extensions.extend(other.extensions);
    }
}

impl DeviceCategory {
    /// Merges the extensions field of another device category into this one.
    /// Does not check whether the two device categories share the same ID and other metadata.
    pub fn merge(&mut self, other: DeviceCategory) {
        self.extensions.extend(other.extensions);
    }
}
