use std::collections::HashSet;

use anyhow::anyhow;
use semver::Version;
use surrealdb::sql::{Id, Thing};

use super::common::{
    Device, DeviceClassification, DeviceClassificationUniqueID, DeviceManufacturer,
    DeviceManufacturerUniqueID, InventoryExtensionMetadata, InventoryExtensionUniqueID, UniqueID,
};
use super::database::{
    DeviceClassificationPullRecord, DeviceClassificationPushRecord, DeviceManufacturerPullRecord,
    DeviceManufacturerPushRecord, DevicePullRecord, DevicePushRecord,
    InventoryExtensionMetadataPullRecord, InventoryExtensionMetadataPushRecord,
};
use crate::database::{
    DEVICE_CLASSIFICATION_TABLE_NAME, DEVICE_MANUFACTURER_TABLE_NAME, EXTENSION_TABLE_NAME,
};

impl<'a> From<&'a InventoryExtensionMetadata> for InventoryExtensionMetadataPushRecord<'a> {
    fn from(extension: &'a InventoryExtensionMetadata) -> Self {
        InventoryExtensionMetadataPushRecord {
            id: Thing::from(&extension.id),
            display_name: &extension.display_name,
            version: extension.version.to_string(),
        }
    }
}

impl TryFrom<InventoryExtensionMetadataPullRecord> for InventoryExtensionMetadata {
    type Error = anyhow::Error;
    fn try_from(extension: InventoryExtensionMetadataPullRecord) -> Result<Self, anyhow::Error> {
        Ok(InventoryExtensionMetadata {
            id: InventoryExtensionUniqueID::try_from(extension.id)?,
            display_name: extension.display_name,
            version: Version::parse(&extension.version)?,
        })
    }
}

impl<'a> From<&'a DeviceManufacturer> for DeviceManufacturerPushRecord<'a> {
    fn from(manufacturer: &'a DeviceManufacturer) -> Self {
        DeviceManufacturerPushRecord {
            id: Thing::from(&manufacturer.id),
            display_name: &manufacturer.display_name,
            extensions: manufacturer.extensions.iter().map(Thing::from).collect(),
        }
    }
}

impl TryFrom<DeviceManufacturerPullRecord> for DeviceManufacturer {
    type Error = anyhow::Error;
    fn try_from(manufacturer: DeviceManufacturerPullRecord) -> Result<Self, anyhow::Error> {
        Ok(DeviceManufacturer {
            id: DeviceManufacturerUniqueID::try_from(manufacturer.id)?,
            display_name: manufacturer.display_name,
            extensions: manufacturer
                .extensions
                .into_iter()
                .map(InventoryExtensionUniqueID::try_from)
                .collect::<Result<HashSet<_>, _>>()?,
        })
    }
}

impl<'a> From<&'a DeviceClassification> for DeviceClassificationPushRecord<'a> {
    fn from(classification: &'a DeviceClassification) -> Self {
        DeviceClassificationPushRecord {
            id: Thing::from(&classification.id),
            display_name: &classification.display_name,
            extensions: classification.extensions.iter().map(Thing::from).collect(),
        }
    }
}

impl TryFrom<DeviceClassificationPullRecord> for DeviceClassification {
    type Error = anyhow::Error;
    fn try_from(classification: DeviceClassificationPullRecord) -> Result<Self, anyhow::Error> {
        Ok(DeviceClassification {
            id: DeviceClassificationUniqueID::try_from(classification.id)?,
            display_name: classification.display_name,
            extensions: classification
                .extensions
                .into_iter()
                .map(InventoryExtensionUniqueID::try_from)
                .collect::<Result<HashSet<_>, _>>()?,
        })
    }
}

impl<'a> From<&'a Device> for DevicePushRecord<'a> {
    fn from(device: &'a Device) -> Self {
        DevicePushRecord {
            internal_id: &device.internal_id,
            display_name: &device.display_name,
            manufacturer: Thing::from(&device.manufacturer),
            classification: Thing::from(&device.classification),
            extension: Thing::from(&device.extension),
            primary_model_identifiers: &device.primary_model_identifiers,
            extended_model_identifiers: &device.extended_model_identifiers,
        }
    }
}

impl TryFrom<DevicePullRecord> for Device {
    type Error = anyhow::Error;
    fn try_from(device: DevicePullRecord) -> Result<Self, Self::Error> {
        Ok(Device {
            internal_id: device.internal_id,
            display_name: device.display_name,
            manufacturer: DeviceManufacturerUniqueID::try_from(device.manufacturer)?,
            classification: DeviceClassificationUniqueID::try_from(device.classification)?,
            extension: InventoryExtensionUniqueID::try_from(device.extension)?,
            primary_model_identifiers: device.primary_model_identifiers,
            extended_model_identifiers: device.extended_model_identifiers,
        })
    }
}

impl From<&InventoryExtensionUniqueID> for Thing {
    fn from(id: &InventoryExtensionUniqueID) -> Self {
        Thing {
            tb: EXTENSION_TABLE_NAME.to_owned(),
            id: Id::String(id.unnamespaced().to_owned()),
        }
    }
}

impl TryFrom<Thing> for InventoryExtensionUniqueID {
    type Error = anyhow::Error;
    fn try_from(thing: Thing) -> Result<Self, Self::Error> {
        if let Id::String(id) = thing.id {
            Ok(InventoryExtensionUniqueID::new(id))
        } else {
            Err(anyhow!("Non-string ID for extension"))
        }
    }
}

impl From<&DeviceManufacturerUniqueID> for Thing {
    fn from(id: &DeviceManufacturerUniqueID) -> Self {
        Thing {
            tb: DEVICE_MANUFACTURER_TABLE_NAME.to_owned(),
            id: Id::String(id.unnamespaced().to_owned()),
        }
    }
}

impl TryFrom<Thing> for DeviceManufacturerUniqueID {
    type Error = anyhow::Error;
    fn try_from(thing: Thing) -> Result<Self, Self::Error> {
        if let Id::String(id) = thing.id {
            Ok(DeviceManufacturerUniqueID::new(id))
        } else {
            Err(anyhow!("Non-string ID for device manufacturer"))
        }
    }
}

impl From<&DeviceClassificationUniqueID> for Thing {
    fn from(id: &DeviceClassificationUniqueID) -> Self {
        Thing {
            tb: DEVICE_CLASSIFICATION_TABLE_NAME.to_owned(),
            id: Id::String(id.unnamespaced().to_owned()),
        }
    }
}

impl TryFrom<Thing> for DeviceClassificationUniqueID {
    type Error = anyhow::Error;
    fn try_from(thing: Thing) -> Result<Self, Self::Error> {
        if let Id::String(id) = thing.id {
            Ok(DeviceClassificationUniqueID::new(id))
        } else {
            Err(anyhow!("Non-string ID for device classification"))
        }
    }
}
