use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::DirEntry;
use std::path::Path;
use std::str::FromStr;

use log::{error, info, warn};
use semver::Version;
use serde::Deserialize;

use super::conflicts::{LoadConflict, StageConflict};
use super::{ExtensionID, Metadata};
use crate::database::Database;
use crate::models::common::{
    Device, DeviceClassification, DeviceClassificationUniqueID, DeviceManufacturer,
    DeviceManufacturerUniqueID, UniqueID,
};
use crate::{stop, Context, Override};

/// An extension of the database inventory system.
#[derive(Debug, Clone)]
pub struct InventoryExtension {
    pub metadata: Metadata,
    pub device_manufacturers: Vec<DeviceManufacturer>,
    pub device_classifications: Vec<DeviceClassification>,
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
    device_classifications: Option<Vec<DeviceClassificationToml>>,
    devices: Vec<DeviceToml>,
}

/// A device manufacturer as read from a TOML extension.
/// This must be converted into a [`DeviceManufacturer`] before adding it to the database.
#[derive(Debug, Deserialize)]
struct DeviceManufacturerToml {
    id: String,
    display_name: String,
}

/// A classification of device as read from a TOML extension.
/// This must be converted into a [`DeviceClassification`] before adding it to the database.
#[derive(Debug, Deserialize)]
struct DeviceClassificationToml {
    id: String,
    display_name: String,
}

/// A device and its metadata as read from a TOML extension.
/// This must be converted into a [`Device`] before adding it to the database.
#[derive(Debug, Deserialize)]
struct DeviceToml {
    internal_id: String,
    display_name: String,
    manufacturer: String,
    classification: String,
    primary_model_identifiers: Vec<String>,
    extended_model_identifiers: Vec<String>,
}

/// The mode in which the manager should load extensions and handle conflicts.
#[derive(Debug, PartialEq, Eq)]
enum HandlerMode {
    /// Manual mode requires the user to handle conflicts. All conflicts are logged and the server
    /// is stopped so the conflicts can be resolved manually.
    Manual,
    /// Auto mode automatically handles conflicts. Conflicts are logged and the manager decides
    /// whether to reload conflicting extensions based on the severity of the conflict.
    Auto,
    /// Force-reload mode automatically reloads conflicting extensions. Conflicts are logged and
    /// the manager reloads conflicting extensions regardless of the severity of the conflict.
    ForceReload,
}

impl From<&Option<Override>> for HandlerMode {
    fn from(override_mode: &Option<Override>) -> Self {
        match override_mode {
            Some(mode) => match mode {
                Override::Load => Self::ForceReload,
                Override::Handle => Self::Auto,
            },
            None => Self::Manual,
        }
    }
}

/// Manages the parsing and loading of extensions into the database.
pub struct ExtensionManager {
    staged_extensions: Vec<InventoryExtension>,
    handler_mode: HandlerMode,
}

impl ExtensionManager {
    /// Loads all extensions from the default location (the extensions folder).
    pub fn new(ctx: &Context) -> anyhow::Result<Self> {
        let mut manager = Self::base_with_context(ctx);
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

    /// Creates a manager with the correct handler mode, but with no staged extensions.
    pub(super) fn base_with_context(ctx: &Context) -> Self {
        Self {
            staged_extensions: Vec::new(),
            handler_mode: HandlerMode::from(&ctx.override_mode),
        }
    }

    /// Parses a TOML file into an extensoin which can be added to the database by the manager.
    fn parse_extension(filename: &Path) -> anyhow::Result<InventoryExtension> {
        let toml = std::fs::read_to_string(filename)?;
        let extension_toml: InventoryExtensionToml = toml::from_str(&toml)?;
        Ok(InventoryExtension::from(extension_toml))
    }

    /// Stages an extension, checking whether it conflicts with other already-staged extensions.
    pub(super) fn stage_extension(
        &mut self,
        extension: InventoryExtension,
    ) -> anyhow::Result<Option<StageConflict>> {
        if !self.already_contains(&extension) {
            info!(
                "Staging extension '{}'.",
                extension.metadata.id.unnamespaced()
            );
            self.staged_extensions.push(extension);
        } else {
            // $ NOTIFICATION OR PROMPT HERE
            error!(
                "Extension with ID '{}' already staged, skipping.",
                extension.metadata.id.unnamespaced()
            );
            return Ok(Some(StageConflict::new(&extension.metadata)));
        }

        Ok(None)
    }

    /// Checks whether a given extension shares an ID with any of the already-staged extensions.
    fn already_contains(&self, extension: &InventoryExtension) -> bool {
        let extension_id = &extension.metadata.id;
        for staged_extension in &self.staged_extensions {
            let staged_extension_id = &staged_extension.metadata.id;
            if extension_id == staged_extension_id {
                return true;
            }
        }

        false
    }

    /// Adds all extensions from the manager into the database, handling any conflicts.
    pub async fn load_extensions(self, db: &Database) -> anyhow::Result<Vec<LoadConflict>> {
        info!("Loading staged inventory extensions into database...");

        let mut loaded_extensions = db.list_extensions().await?;
        let mut conflicts = Vec::new();
        let mut should_crash = false;
        'current_extension: for staged_extension in self.staged_extensions.into_iter() {
            let staged_extension_metadata = &staged_extension.metadata;
            let staged_extension_id = staged_extension_metadata.id.unnamespaced().to_owned();

            let Some(conflict) = LoadConflict::new(&staged_extension, &mut loaded_extensions)
            else {
                info!("Loading extension '{}'...", staged_extension_id);
                db.load_extension(staged_extension).await?;
                info!("Successfully loaded extension '{}'.", staged_extension_id);
                continue 'current_extension;
            };

            match self.handler_mode {
                HandlerMode::Manual => {
                    // If the conflict would generally be handled with a reload, the user will be
                    // given an error log explaining the conflict. The server will crash after all
                    // the conflicts have been logged, which is why this uses a flag instead of an
                    // immediate call to [`stop`].
                    conflict.log(false);
                    if conflict.should_reload() {
                        should_crash = true;
                    }
                }
                HandlerMode::Auto => {
                    conflict.log(true);
                    if conflict.should_reload() {
                        info!("Reloading extension '{}'...", staged_extension_id);
                        db.reload_extension(staged_extension).await?;
                        info!("Successfully reloaded extension '{}'.", staged_extension_id);
                    }
                }
                HandlerMode::ForceReload => {
                    warn!("Force-reloading extension '{}'...", staged_extension_id);
                    db.reload_extension(staged_extension).await?;
                    info!("Successfully reloaded extension '{}'.", staged_extension_id);
                }
            }

            conflicts.push(conflict);
        }

        match should_crash {
            true => {
                error!("Please resolve extension conflicts before restarting server.");
                stop(5);
            }
            false => info!("All staged extensions successfully loaded."),
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

impl InventoryExtension {
    /// Creates the builtin extension, which is added to the database to set up reserved items.
    pub fn builtin() -> Self {
        let id = ExtensionID::new("builtin");

        let metadata = Metadata {
            id: ExtensionID::new("builtin"),
            display_name: "Built-in".to_owned(),
            version: Version::new(0, 0, 0),
        };

        let device_manufacturers = vec![
            DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new("apple"),
                display_name: "Apple".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new("samsung"),
                display_name: "Samsung".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new("google"),
                display_name: "Google".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new("motorola"),
                display_name: "Motorola".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new("dell"),
                display_name: "Dell".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new("hp"),
                display_name: "HP".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceManufacturer {
                id: DeviceManufacturerUniqueID::new("lenovo"),
                display_name: "Lenovo".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
        ];

        let device_classifications = vec![
            DeviceClassification {
                id: DeviceClassificationUniqueID::new("phone"),
                display_name: "Phone".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceClassification {
                id: DeviceClassificationUniqueID::new("tablet"),
                display_name: "Tablet".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceClassification {
                id: DeviceClassificationUniqueID::new("console"),
                display_name: "console".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceClassification {
                id: DeviceClassificationUniqueID::new("laptop"),
                display_name: "Laptop".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
            DeviceClassification {
                id: DeviceClassificationUniqueID::new("desktop"),
                display_name: "Desktop".to_owned(),
                extensions: HashSet::from([id.clone()]),
            },
        ];

        Self {
            metadata,
            device_manufacturers,
            device_classifications,
            devices: Vec::new(),
        }
    }
}

// TODO: Remove unwraps
// * Inner types here ([`DeviceManufacturer`], [`DeviceClassification`], [`Device`]) must be
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

        let device_classifications = toml
            .device_classifications
            .unwrap_or_default()
            .into_iter()
            .map(|c| DeviceClassification {
                id: DeviceClassificationUniqueID::new(&c.id),
                display_name: c.display_name,
                extensions: HashSet::from([ExtensionID::new(&toml.extension_id)]),
            })
            .collect();

        let devices = toml
            .devices
            .into_iter()
            // ? Is there a more conventional way to do this conversion?
            .map(|d| Device {
                internal_id: d.internal_id,
                display_name: d.display_name,
                manufacturer: DeviceManufacturerUniqueID::new(&d.manufacturer),
                classification: DeviceClassificationUniqueID::new(&d.classification),
                extension: ExtensionID::new(&toml.extension_id),
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
            device_classifications,
            devices,
        }
    }
}
