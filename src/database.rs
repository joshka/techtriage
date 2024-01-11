use std::net::{Ipv4Addr, SocketAddr};

use futures_util::future;
use log::{debug, error, info};
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::Root;
use surrealdb::Surreal;

use crate::extensions::InventoryExtension;
use crate::models::common::{
    Device, DeviceCategory, DeviceCategoryUniqueID, DeviceManufacturer, DeviceManufacturerUniqueID,
    DeviceUniqueID, InventoryExtensionMetadata, InventoryExtensionUniqueID, UniqueID,
};
use crate::models::database::{
    DeviceCategoryPullRecord, DeviceCategoryPushRecord, DeviceManufacturerPullRecord,
    DeviceManufacturerPushRecord, DevicePullRecord, DevicePushRecord, GenericPullRecord,
    InventoryExtensionMetadataPullRecord, InventoryExtensionMetadataPushRecord,
};
use crate::stop;

pub const EXTENSION_TABLE_NAME: &str = "extensions";
pub const DEVICE_MANUFACTURER_TABLE_NAME: &str = "device_manufacturers";
pub const DEVICE_CATEGORY_TABLE_NAME: &str = "device_categories";
pub const DEVICE_TABLE_NAME: &str = "devices";

/// Wrapper type for a SurrealDB connection.
pub struct Database {
    connection: Surreal<Client>,
    #[allow(dead_code)]
    config: DatabaseConfig,
}

/// Configuration for connecting to the database.
pub struct DatabaseConfig {
    pub address: SocketAddr,
    pub username: String,
    pub password: String,
    pub namespace: String,
    pub database: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        DatabaseConfig {
            address: (Ipv4Addr::LOCALHOST, 8000).into(),
            username: "root".to_owned(),
            password: "root".to_owned(),
            namespace: "test".to_owned(),
            database: "test".to_owned(),
        }
    }
}

impl Database {
    /// Connects to the database, if it is available, using the default configuration.
    pub async fn connect() -> Self {
        Self::connect_with_config(DatabaseConfig::default()).await
    }

    /// Connects to the database using defaults except for the provided database name.
    #[cfg(test)]
    pub async fn connect_with_name(database: &str) -> Self {
        Self::connect_with_config(DatabaseConfig {
            database: database.to_owned(),
            ..Default::default()
        })
        .await
    }

    /// Connects to the database using the provided configuration.
    pub async fn connect_with_config(config: DatabaseConfig) -> Self {
        info!("Connecting and authenticating to database...");
        debug!(
            "Using namespace '{}' and database '{}' at address {}.",
            config.namespace, config.database, config.address
        );

        let Ok(connection) = Surreal::new::<Ws>(config.address).await else {
            error!("Failed to connect to database. Please make sure it is running.");
            stop(1);
        };

        connection
            .use_ns(&config.namespace)
            .use_db(&config.database)
            .await
            .unwrap_or_else(|_| {
                error!("Failed to select namespace and database from SurrealDB instance.");
                stop(2);
            });

        connection
            .signin(Root {
                username: &config.username,
                password: &config.password,
            })
            .await
            .unwrap_or_else(|_| {
                error!("Failed to sign into SurrealDB instance. Please check your credentials.");
                stop(3);
            });

        info!("Database connection established.");

        Self { connection, config }
    }

    /// Sets up the tables and schema needed for core functionality.
    /// If the tables already exist, this will do nothing.
    pub async fn setup_tables(&self) -> anyhow::Result<()> {
        info!("Setting up database tables/schema...");

        // * ID is an implicit field on all tables and uses the [`sql::Thing`] type.
        self.connection
            .query(&format!(
                "
                DEFINE TABLE {EXTENSION_TABLE_NAME} SCHEMAFUL;
                DEFINE FIELD display_name ON TABLE {EXTENSION_TABLE_NAME} TYPE string;
                DEFINE FIELD version ON TABLE {EXTENSION_TABLE_NAME} TYPE string;

                DEFINE TABLE {DEVICE_MANUFACTURER_TABLE_NAME} SCHEMAFUL;
                DEFINE FIELD display_name ON TABLE {DEVICE_MANUFACTURER_TABLE_NAME} TYPE string;
                DEFINE FIELD extensions ON TABLE {DEVICE_MANUFACTURER_TABLE_NAME} TYPE array<record({EXTENSION_TABLE_NAME})>;
                DEFINE FIELD extensions.* ON TABLE {DEVICE_MANUFACTURER_TABLE_NAME} TYPE record({EXTENSION_TABLE_NAME});

                DEFINE TABLE {DEVICE_CATEGORY_TABLE_NAME} SCHEMAFUL;
                DEFINE FIELD display_name ON TABLE {DEVICE_CATEGORY_TABLE_NAME} TYPE string;
                DEFINE FIELD extensions ON TABLE {DEVICE_CATEGORY_TABLE_NAME} TYPE array<record({EXTENSION_TABLE_NAME})>;
                DEFINE FIELD extensions.* ON TABLE {DEVICE_CATEGORY_TABLE_NAME} TYPE record({EXTENSION_TABLE_NAME});

                DEFINE TABLE {DEVICE_TABLE_NAME} SCHEMAFUL;
                DEFINE FIELD display_name ON TABLE {DEVICE_TABLE_NAME} TYPE string;
                DEFINE FIELD manufacturer ON TABLE {DEVICE_TABLE_NAME} TYPE record({DEVICE_MANUFACTURER_TABLE_NAME});
                DEFINE FIELD category ON TABLE {DEVICE_TABLE_NAME} TYPE record({DEVICE_CATEGORY_TABLE_NAME});
                DEFINE FIELD extensions ON TABLE {DEVICE_TABLE_NAME} TYPE array<record({EXTENSION_TABLE_NAME})>;
                DEFINE FIELD extensions.* ON TABLE {DEVICE_TABLE_NAME} TYPE record({EXTENSION_TABLE_NAME});
                DEFINE FIELD primary_model_identifiers ON TABLE {DEVICE_TABLE_NAME} TYPE array<string>;
                DEFINE FIELD primary_model_identifiers.* ON TABLE {DEVICE_TABLE_NAME} TYPE string;
                DEFINE FIELD extended_model_identifiers ON TABLE {DEVICE_TABLE_NAME} TYPE array<string>;
                DEFINE FIELD extended_model_identifiers.* ON TABLE {DEVICE_TABLE_NAME} TYPE string;
                ",
            ))
            .await
            .unwrap_or_else(|_| {
                error!("Failed to set up database tables/schema.");
                stop(4);
            });

        Ok(())
    }

    /// Deletes the current database and all of its contents.
    /// Used by tests so the database instance can be reused.
    #[cfg(test)]
    pub async fn teardown(self) {
        self.connection
            .query(&format!("REMOVE DATABASE {}", self.config.database))
            .await
            .unwrap();
    }

    /// Loads the contents of an inventory extension into the database.
    pub async fn load_extension(&self, extension: InventoryExtension) -> surrealdb::Result<()> {
        self.connection
            .create::<Vec<GenericPullRecord>>(EXTENSION_TABLE_NAME)
            .content(InventoryExtensionMetadataPushRecord::from(
                &extension.metadata,
            ))
            .await?;

        let mut futures = Vec::new();
        for category in extension.device_categories {
            futures.push(self.add_device_category(category));
        }
        future::join_all(futures).await;

        let mut futures = Vec::new();
        for manufacturer in extension.device_manufacturers {
            futures.push(self.add_device_manufacturer(manufacturer));
        }
        future::join_all(futures).await;

        let mut futures = Vec::new();
        for device in extension.devices {
            futures.push(self.add_device(device));
        }
        future::join_all(futures).await;

        Ok(())
    }

    /// Removes an extension and its contents from the database.
    pub async fn unload_extension(
        &self,
        extension_id: &InventoryExtensionUniqueID,
    ) -> anyhow::Result<()> {
        self.connection
            .query(&format!(
                "
                DELETE {DEVICE_MANUFACTURER_TABLE_NAME} WHERE extensions = [\"{0}\"];
                DELETE {DEVICE_CATEGORY_TABLE_NAME} WHERE extensions = [\"{0}\"];
                DELETE {DEVICE_TABLE_NAME} WHERE extensions = [\"{0}\"];
                DELETE {EXTENSION_TABLE_NAME} WHERE id = \"{0}\";
                
                UPDATE {DEVICE_MANUFACTURER_TABLE_NAME} SET extensions -= [\"{0}\"];
                UPDATE {DEVICE_CATEGORY_TABLE_NAME} SET extensions -= [\"{0}\"];
                UPDATE {DEVICE_TABLE_NAME} SET extensions -= [\"{0}\"];
                ",
                extension_id.namespaced()
            ))
            .await?;

        Ok(())
    }

    /// Removes the extension corresponding to the ID of the given extension, and loads the given
    /// extension in its place.
    pub async fn reload_extension(&self, extension: InventoryExtension) -> anyhow::Result<()> {
        self.unload_extension(&extension.metadata.id).await?;
        self.load_extension(extension).await?;
        Ok(())
    }

    /// Lists all currently-loaded extensions in the database.
    pub async fn list_extensions(&self) -> anyhow::Result<Vec<InventoryExtensionMetadata>> {
        let pull_records = self
            .connection
            .select::<Vec<InventoryExtensionMetadataPullRecord>>(EXTENSION_TABLE_NAME)
            .await?;

        let mut extensions = Vec::new();
        for record in pull_records {
            extensions.push(InventoryExtensionMetadata::try_from(record)?);
        }

        Ok(extensions)
    }

    /// Lists all the device manufacturers in the database.
    #[allow(dead_code)]
    pub async fn list_device_manufacturers(&self) -> anyhow::Result<Vec<DeviceManufacturer>> {
        let pull_records = self
            .connection
            .select::<Vec<DeviceManufacturerPullRecord>>(DEVICE_MANUFACTURER_TABLE_NAME)
            .await?;

        let mut manufacturers = Vec::new();
        for record in pull_records {
            manufacturers.push(DeviceManufacturer::try_from(record)?);
        }

        Ok(manufacturers)
    }

    /// Lists all the device categories in the database.
    #[allow(dead_code)]
    pub async fn list_device_categories(&self) -> anyhow::Result<Vec<DeviceCategory>> {
        let pull_records = self
            .connection
            .select::<Vec<DeviceCategoryPullRecord>>(DEVICE_CATEGORY_TABLE_NAME)
            .await?;

        let mut categories = Vec::new();
        for record in pull_records {
            categories.push(DeviceCategory::try_from(record)?);
        }

        Ok(categories)
    }

    /// Lists all the devices in the database.
    pub async fn list_devices(&self) -> anyhow::Result<Vec<Device>> {
        let pull_records = self
            .connection
            .select::<Vec<DevicePullRecord>>(DEVICE_TABLE_NAME)
            .await?;

        let mut devices = Vec::new();
        for record in pull_records {
            devices.push(Device::try_from(record)?);
        }

        Ok(devices)
    }

    /// Adds a deivice manufacturer to the database, merging it with an existing record if needed.
    pub async fn add_device_manufacturer(
        &self,
        mut manufacturer: DeviceManufacturer,
    ) -> anyhow::Result<()> {
        if let Some(existing_record) = self.get_device_manufacturer(&manufacturer.id).await? {
            manufacturer.merge(existing_record.try_into()?);
            self.remove_device_manufacturer(&manufacturer.id).await?;
        }

        self.connection
            .create::<Vec<GenericPullRecord>>(DEVICE_MANUFACTURER_TABLE_NAME)
            .content(DeviceManufacturerPushRecord::from(&manufacturer))
            .await?;

        Ok(())
    }

    /// Adds a device category to the database, merging it with an existing record if needed.
    async fn add_device_category(&self, mut category: DeviceCategory) -> anyhow::Result<()> {
        if let Some(existing_record) = self.get_device_category(&category.id).await? {
            category.merge(existing_record.try_into()?);
            self.remove_device_category(&category.id).await?;
        }

        self.connection
            .create::<Vec<GenericPullRecord>>(DEVICE_CATEGORY_TABLE_NAME)
            .content(DeviceCategoryPushRecord::from(&category))
            .await?;

        Ok(())
    }

    /// Adds a device to the database, merging it with an existing record if needed.
    async fn add_device(&self, mut device: Device) -> anyhow::Result<()> {
        if let Some(existing_record) = self.get_device(&device.id).await? {
            device.merge(existing_record.try_into()?);
            self.remove_device(&device.id).await?;
        }

        self.connection
            .create::<Vec<GenericPullRecord>>(DEVICE_TABLE_NAME)
            .content(DevicePushRecord::from(&device))
            .await?;

        Ok(())
    }

    /// Removes a single device manufacturer from the database.
    // TODO: Any way to consolidate these 3 methods?
    pub async fn remove_device_manufacturer(
        &self,
        id: &DeviceManufacturerUniqueID,
    ) -> anyhow::Result<()> {
        self.connection
            .query(&format!("DELETE {}", id.namespaced()))
            .await?;

        Ok(())
    }

    /// Removes a single device category from the database.
    pub async fn remove_device_category(&self, id: &DeviceCategoryUniqueID) -> anyhow::Result<()> {
        self.connection
            .query(&format!("DELETE {}", id.namespaced()))
            .await?;

        Ok(())
    }

    /// Removes a single device from the database.
    pub async fn remove_device(&self, id: &DeviceUniqueID) -> anyhow::Result<()> {
        self.connection
            .query(&format!("DELETE {}", id.namespaced()))
            .await?;

        Ok(())
    }

    // ? Can this be combined with `get_device_category()` into a single function?
    /// Gets a device manufacturer from the database, if it exists.
    async fn get_device_manufacturer(
        &self,
        id: &DeviceManufacturerUniqueID,
    ) -> anyhow::Result<Option<DeviceManufacturerPullRecord>> {
        Ok(self
            .connection
            .select::<Option<DeviceManufacturerPullRecord>>((
                DEVICE_MANUFACTURER_TABLE_NAME,
                id.unnamespaced(),
            ))
            .await?)
    }

    /// Gets a device category from the database, if it exists.
    async fn get_device_category(
        &self,
        id: &DeviceCategoryUniqueID,
    ) -> anyhow::Result<Option<DeviceCategoryPullRecord>> {
        Ok(self
            .connection
            .select::<Option<DeviceCategoryPullRecord>>((
                DEVICE_CATEGORY_TABLE_NAME,
                id.unnamespaced(),
            ))
            .await?)
    }

    /// Gets a device from the database, if it exists.
    async fn get_device(&self, id: &DeviceUniqueID) -> anyhow::Result<Option<DevicePullRecord>> {
        Ok(self
            .connection
            .select::<Option<DevicePullRecord>>((DEVICE_TABLE_NAME, id.unnamespaced()))
            .await?)
    }

    /// Checks that the database contains the given extension and its contents.
    /// Used for testing purposes.
    #[cfg(test)]
    pub async fn contains(&self, extension: &InventoryExtension, exclusive: bool) {
        let loaded_extensions = self.list_extensions().await.unwrap();

        if exclusive {
            assert_eq!(loaded_extensions.len(), 1);
        }

        let loaded_device_manufacturers = self.list_device_manufacturers().await.unwrap();
        let loaded_device_categories = self.list_device_categories().await.unwrap();
        let loaded_devices = self.list_devices().await.unwrap();

        assert!(loaded_extensions.contains(&extension.metadata));

        'extension_manufacturers: for extension_manufacturer in &extension.device_manufacturers {
            for loaded_manufacturer in &loaded_device_manufacturers {
                let same_id = loaded_manufacturer.id == extension_manufacturer.id;
                let same_display_name =
                    loaded_manufacturer.display_name == extension_manufacturer.display_name;
                let correct_extensions = (exclusive
                    && loaded_manufacturer.extensions.len() == 1
                    && loaded_manufacturer
                        .extensions
                        .contains(&extension.metadata.id))
                    || (!exclusive
                        && loaded_manufacturer
                            .extensions
                            .contains(&extension.metadata.id));

                if same_id && same_display_name && correct_extensions {
                    continue 'extension_manufacturers;
                }
            }

            panic!("Device manufacturer not found");
        }

        'extension_categories: for extension_category in &extension.device_categories {
            for loaded_category in &loaded_device_categories {
                let same_id = loaded_category.id == extension_category.id;
                let same_display_name =
                    loaded_category.display_name == extension_category.display_name;
                let correct_extensions = (exclusive
                    && loaded_category.extensions.len() == 1
                    && loaded_category.extensions.contains(&extension.metadata.id))
                    || (!exclusive && loaded_category.extensions.contains(&extension.metadata.id));

                if same_id && same_display_name && correct_extensions {
                    continue 'extension_categories;
                }
            }

            panic!("Device category not found");
        }

        'extension_devices: for extension_device in &extension.devices {
            // TODO: Add checks for primary and extended model identifiers
            for loaded_device in &loaded_devices {
                let same_id = loaded_device.id == extension_device.id;
                let same_display_name = loaded_device.display_name == extension_device.display_name;
                let same_manufacturer = loaded_device.manufacturer == extension_device.manufacturer;
                let same_category = loaded_device.category == extension_device.category;
                let correct_extensions = (exclusive
                    && loaded_device.extensions.len() == 1
                    && loaded_device.extensions.contains(&extension.metadata.id))
                    || (!exclusive && loaded_device.extensions.contains(&extension.metadata.id));

                if same_id
                    && same_display_name
                    && same_manufacturer
                    && same_category
                    && correct_extensions
                {
                    continue 'extension_devices;
                }
            }

            panic!("Device not found");
        }
    }
}
