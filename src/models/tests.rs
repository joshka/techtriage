use std::collections::HashSet;

use super::common::{
    Device, DeviceCategory, DeviceCategoryUniqueID, DeviceManufacturer, DeviceManufacturerUniqueID,
    DeviceUniqueID, InventoryExtensionUniqueID, UniqueID,
};

impl DeviceManufacturer {
    /// Creates a basic device manufacturer for testing purposes.
    /// Can be modified to test different scenarios.
    pub fn test(num: u32, extension_id: &InventoryExtensionUniqueID) -> Self {
        Self {
            id: DeviceManufacturerUniqueID::new(format!("test_{num}")),
            display_name: format!("Test Device Manufacturer {num}"),
            extensions: HashSet::from([extension_id.clone()]),
        }
    }
}

impl DeviceCategory {
    /// Creates a basic device category for testing purposes.
    /// Can be modified to test different scenarios.
    pub fn test(num: u32, extension_id: &InventoryExtensionUniqueID) -> Self {
        Self {
            id: DeviceCategoryUniqueID::new(format!("test_{num}")),
            display_name: format!("Test Device Category {num}"),
            extensions: HashSet::from([extension_id.clone()]),
        }
    }
}

impl Device {
    /// Creates a basic device for testing purposes.
    /// Can be modified to test different scenarios.
    pub fn test(
        num: u32,
        extension_id: &InventoryExtensionUniqueID,
        manufacturer_id: &DeviceManufacturerUniqueID,
        category_id: &DeviceCategoryUniqueID,
    ) -> Self {
        Self {
            id: DeviceUniqueID::new(format!("test_{num}")),
            display_name: format!("Test Device {num}"),
            manufacturer: manufacturer_id.clone(),
            category: category_id.clone(),
            extensions: HashSet::from([extension_id.clone()]),
            primary_model_identifiers: vec![format!("test_{num}_primary")],
            extended_model_identifiers: vec![format!("test_{num}_extended")],
        }
    }
}
