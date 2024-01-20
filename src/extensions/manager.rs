use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::DirEntry;
use std::path::Path;
use std::str::FromStr;

use log::{info, warn};
use semver::Version;
use serde::Deserialize;

use super::conflicts::LoadConflict;
use super::{ExtensionID, Metadata};
use crate::database::Database;
use crate::models::common::{
    Device, DeviceCategory, DeviceCategoryUniqueID, DeviceManufacturer, DeviceManufacturerUniqueID,
    DeviceUniqueID, UniqueID,
};

/// An extension of the database inventory system.
#[derive(Debug, Clone)]
pub struct InventoryExtension {
    pub metadata: Metadata,
    pub device_manufacturers: Vec<DeviceManufacturer>,
    pub device_categories: Vec<DeviceCategory>,
    pub devices: Vec<Device>,
}

/// An inventory extension as read from a TOML file.
/// Some types are not compatible with the database, so this type must be converted into an
/// [`InventoryExtension`] before calling [`Database::load_extension`].
#[derive(Debug, Deserialize)]
struct InventoryExtensionToml {
    extension_id: String,
    extension_display_name: String,
    extension_version: String,
    device_manufacturers: Option<Vec<DeviceManufacturerToml>>,
    device_categories: Option<Vec<DeviceCategoryToml>>,
    devices: Vec<DeviceToml>,
}

/// A device manufacturer as read from a TOML extension.
/// This must be converted into a [`DeviceManufacturer`] before adding it to the database.
#[derive(Debug, Deserialize)]
struct DeviceManufacturerToml {
    id: String,
    display_name: String,
}

/// A category of device as read from a TOML extension.
/// This must be converted into a [`DeviceCategory`] before adding it to the database.
#[derive(Debug, Deserialize)]
struct DeviceCategoryToml {
    id: String,
    display_name: String,
}

/// A device and its metadata as read from a TOML extension.
/// This must be converted into a [`Device`] before adding it to the database.
#[derive(Debug, Deserialize)]
struct DeviceToml {
    id: String,
    display_name: String,
    manufacturer: String,
    category: String,
    primary_model_identifiers: Vec<String>,
    extended_model_identifiers: Vec<String>,
}

/// Manages the parsing and loading of extensions into the database.
pub struct ExtensionManager {
    staged_extensions: Vec<InventoryExtension>,
    auto_reload: bool,
}

impl ExtensionManager {
    /// Loads all extensions from the default location (the extensions folder).
    pub fn new(auto_reload: bool) -> anyhow::Result<Self> {
        let mut manager = Self::base_with_context(auto_reload);
        for extension_file in std::fs::read_dir("./extensions")?.flatten() {
            if Self::is_extension(&extension_file) {
                info!(
                    "Located extension file: {}",
                    extension_file.path().display()
                );
                manager.stage_extension(Self::parse_extension(&extension_file.path())?)?;
            }
        }

        Ok(manager)
    }

    /// Creates a manager with no staged extensions.
    pub fn base_with_context(auto_reload: bool) -> Self {
        Self {
            staged_extensions: Vec::new(),
            auto_reload,
        }
    }

    /// Parses a TOML file into an extension which can be added to the database by the manager.
    fn parse_extension(filename: &Path) -> anyhow::Result<InventoryExtension> {
        let toml = std::fs::read_to_string(filename)?;
        let extension_toml: InventoryExtensionToml = toml::from_str(&toml)?;
        Ok(InventoryExtension::from(extension_toml))
    }

    /// Stages an extension.
    pub fn stage_extension(&mut self, extension: InventoryExtension) -> anyhow::Result<()> {
        info!(
            "Staging extension '{}'.",
            extension.metadata.id.unnamespaced()
        );
        self.staged_extensions.push(extension);

        Ok(())
    }

    /// Adds all extensions from the manager into the database, handling any conflicts.
    pub async fn load_extensions(self, db: &Database) -> anyhow::Result<Vec<LoadConflict>> {
        info!("Loading staged inventory extensions into database...");

        let mut loaded_extensions = db.list_extensions().await?;
        let mut conflicts = Vec::new();
        for staged_extension in self.staged_extensions.into_iter() {
            let staged_extension_metadata = &staged_extension.metadata;
            let staged_extension_id = staged_extension_metadata.id.unnamespaced().to_owned();

            let Some(conflict) = LoadConflict::new(&staged_extension, &mut loaded_extensions)
            else {
                info!("Loading extension '{}'...", staged_extension_id);
                db.load_extension(staged_extension).await?;
                info!("Successfully loaded extension '{}'.", staged_extension_id);
                continue;
            };

            if self.auto_reload {
                warn!("Force-reloading extension '{}'...", staged_extension_id);
                db.reload_extension(staged_extension).await?;
                info!("Successfully reloaded extension '{}'.", staged_extension_id);
            } else if conflict.should_reload() {
                info!("Reloading extension '{}'...", staged_extension_id);
                db.reload_extension(staged_extension).await?;
                info!("Successfully reloaded extension '{}'.", staged_extension_id);
            } else {
                info!(
                    "Skipping extension '{}' because its version has not changed.",
                    staged_extension_id
                );
            }

            conflicts.push(conflict);
        }

        Ok(conflicts)
    }

    /// Checks whether a given filesystem object is a valid extension.
    fn is_extension(object: &DirEntry) -> bool {
        let (path, filetype) = (object.path(), object.file_type());
        if let Ok(filetype) = filetype {
            if filetype.is_file() && path.extension() == Some(OsStr::new("toml")) {
                return true;
            }
        }

        false
    }
}

// TODO: Remove unwraps
// * Inner types here ([`DeviceManufacturer`], [`DeviceCategory`], [`Device`]) must be
// * converted with context provided by the [`ExtensionToml`] itself, so they cannot be converted
// * directly.
impl From<InventoryExtensionToml> for InventoryExtension {
    fn from(toml: InventoryExtensionToml) -> Self {
        let device_manufacturers = toml
            .device_manufacturers
            .unwrap_or_default()
            .into_iter()
            .map(|m| DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new(&m.id),
                display_name: m.display_name,
                extensions: HashSet::from([ExtensionID::new(&toml.extension_id)]),
            })
            .collect();

        let device_categories = toml
            .device_categories
            .unwrap_or_default()
            .into_iter()
            .map(|c| DeviceCategory {
                id: DeviceCategoryUniqueID::new(&c.id),
                display_name: c.display_name,
                extensions: HashSet::from([ExtensionID::new(&toml.extension_id)]),
            })
            .collect();

        let devices = toml
            .devices
            .into_iter()
            // ? Is there a more conventional way to do this conversion?
            .map(|d| Device {
                id: DeviceUniqueID::new(&d.id),
                display_name: d.display_name,
                manufacturer: DeviceManufacturerUniqueID::new(&d.manufacturer),
                category: DeviceCategoryUniqueID::new(&d.category),
                extensions: HashSet::from([ExtensionID::new(&toml.extension_id)]),
                primary_model_identifiers: d.primary_model_identifiers,
                extended_model_identifiers: d.extended_model_identifiers,
            })
            .collect();

        InventoryExtension {
            metadata: Metadata {
                id: ExtensionID::new(&toml.extension_id),
                display_name: toml.extension_display_name,
                version: Version::from_str(&toml.extension_version).unwrap(),
            },
            device_manufacturers,
            device_categories,
            devices,
        }
    }
}
