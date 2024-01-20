use semver::Version;

use super::conflicts::LoadConflict;
use super::{Extension, ExtensionID, ExtensionManager as Manager, Metadata};
use crate::database::Database;
use crate::models::common::{Device, DeviceCategory, DeviceManufacturer, UniqueID};

/// Tests that an extension will be loaded normally if it does not conflict with an existing
/// extension, regardless of whether the auto-reload flag is set.
#[tokio::test]
async fn load_new_extension() {
    let db = Database::connect_with_name("load_new_extension").await;
    db.setup_tables().await.unwrap();

    // Create a basic extension
    let extension = Extension::test_single(1, 1);

    // Check for conflicts when loading the extension without the auto-reload flag set
    load_and_check_no_conflicts(&db, false, &extension, true, true).await;

    // Remove the extension so the same test can be performed with the auto-reload flag set
    db.unload_extension(&extension.metadata.id).await.unwrap();

    // Check for conflicts when loading the extension with the auto-reload flag set
    load_and_check_no_conflicts(&db, true, &extension, true, true).await;

    db.teardown().await;
}

/// Tests that a conflicting extension which has the same version as an existing extension will be
/// skipped if the auto-reload flag is not set.
#[tokio::test]
async fn skip_duplicate() {
    let db = Database::connect_with_name("skip_duplicate").await;
    db.setup_tables().await.unwrap();

    // Create two extensions with the same metadata, but different contents
    let (original_extension, skipped_extension) = Extension::test_pair_same_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, false, &original_extension, true, false).await;

    // Attempt to load the second extension into the database
    let manager = Manager::with_extensions(false, [skipped_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::already_loaded(original_extension.metadata.id.clone())
    );

    // Make sure that the original extension was not reloaded
    db.contains(&original_extension, true).await;

    db.teardown().await;
}

/// Tests that a conflicting extension which has a different version than an existing extension will
/// be reloaded if the auto-reload flag is not set.
#[tokio::test]
async fn version_change() {
    let db = Database::connect_with_name("version_change").await;
    db.setup_tables().await.unwrap();

    // Create two extensions with different versions and contents
    let (original_extension, updated_extension) = Extension::test_pair_different_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, false, &original_extension, true, false).await;

    // Load the updated extension into the database, which should replace the original extension
    let manager = Manager::with_extensions(false, [updated_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::version_change(original_extension.metadata.id)
    );

    // Make sure that the original extension was reloaded
    db.contains(&updated_extension, true).await;

    db.teardown().await;
}

/// Tests that an extension which conflicts with an existing extension will be reloaded
/// automatically if the auto-reload flag is set.
#[tokio::test]
async fn auto_reload() {
    let db = Database::connect_with_name("auto_reload").await;
    db.setup_tables().await.unwrap();

    // Create two extensions with the same metadata, but different contents
    let (original_extension, reloaded_extension) = Extension::test_pair_same_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, true, &original_extension, true, false).await;

    // Load the second extension into the database, which should replace the original extension
    let manager = Manager::with_extensions(true, [reloaded_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::already_loaded(original_extension.metadata.id)
    );

    // Make sure the original extension was unloaded and the new version was loaded
    db.contains(&reloaded_extension, true).await;
    // Remove the extension so a case with conflicts can be tested
    db.unload_extension(&reloaded_extension.metadata.id)
        .await
        .unwrap();

    // Create two extensions with the same ID, but different versions and different contents
    let (original_extension, reloaded_extension) = Extension::test_pair_different_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, true, &original_extension, true, false).await;

    // Load the second extension into the database, which should replace the original extension
    let manager = Manager::with_extensions(true, [reloaded_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::version_change(original_extension.metadata.id)
    );

    // Make sured that the original extension was unloaded and the new version was loaded
    db.contains(&reloaded_extension, true).await;

    db.teardown().await;
}

/// Tests that extensions are unloaded correctly.
#[tokio::test]
async fn unload_extension() {
    let db = Database::connect_with_name("unload_extension").await;
    db.setup_tables().await.unwrap();

    // Create two extensions with different names but the same contents
    let (extension_1, extension_2) = Extension::test_pair_same_contents();

    // Check that the first extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, false, &extension_1, true, false).await;

    // Check that the second extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, false, &extension_2, false, false).await;

    // Unload the first extension
    db.unload_extension(&extension_1.metadata.id).await.unwrap();

    // Make sure the first extension was unloaded and only the second extension remains
    db.contains(&extension_2, true).await;

    db.teardown().await;
}

/// Tests that an extension can be loaded without generating any conflicts.
/// This test is meant to be a shortcut used by other tests, rather than a standalone test.
async fn load_and_check_no_conflicts(
    db: &Database,
    auto_reload: bool,
    extension: &Extension,
    only_extension: bool,
    remove_after: bool,
) {
    // Load the extension into the database
    let manager = Manager::with_extensions(auto_reload, [extension.clone()]);
    let load_conflicts = manager.load_extensions(db).await.unwrap();
    // Make sure there were no conflicts
    assert!(load_conflicts.is_empty());
    // Make sure the extension was loaded correctly
    // * The additional check for exclusivity is not entirely necessary, but it is included to
    // * provide some extra certainty of the result.
    db.contains(extension, only_extension).await;

    // Remove the extension if requested
    if remove_after {
        db.unload_extension(&extension.metadata.id).await.unwrap();
    }
}

impl Extension {
    /// Creates a basic extension with no contents for testing purposes.
    /// Can be modified to test different scenarios.
    fn test(num: u32) -> Self {
        Self {
            metadata: Metadata {
                id: ExtensionID::new(format!("test_{num}")),
                display_name: format!("Test Extension {num}"),
                version: Version::new(1, 0, 0),
            },
            device_manufacturers: Vec::new(),
            device_categories: Vec::new(),
            devices: Vec::new(),
        }
    }

    /// Creates a single basic extension with contents.
    /// Can be modified to test different scenarios.
    fn test_single(extension_num: u32, contents_num: u32) -> Self {
        // Create an empty extension.
        let mut extension = Self::test(extension_num);

        // Populate the extension with one device manufacturer, device category, and device.
        let device_manufacturer = DeviceManufacturer::test(contents_num, &extension.metadata.id);
        let device_category = DeviceCategory::test(contents_num, &extension.metadata.id);
        let device = Device::test(
            contents_num,
            &extension.metadata.id,
            &device_manufacturer.id,
            &device_category.id,
        );

        extension.device_manufacturers.push(device_manufacturer);
        extension.device_categories.push(device_category);
        extension.devices.push(device);

        extension
    }

    /// Creates two basic extensions with the same metadata and different contents.
    fn test_pair_same_metadata() -> (Self, Self) {
        (Self::test_single(1, 1), Self::test_single(1, 2))
    }

    /// Creates two basic extensions with the same ID, a different version, and different contents.
    fn test_pair_different_metadata() -> (Self, Self) {
        let extension_1 = Self::test_single(1, 1);
        let mut extension_2 = Self::test_single(1, 2);
        extension_2.metadata.version = Version::new(1, 0, 1);

        (extension_1, extension_2)
    }

    /// Creates two basic extensions with a different ID, the same version, and the same contents.
    fn test_pair_same_contents() -> (Self, Self) {
        (Self::test_single(1, 1), Self::test_single(2, 1))
    }
}

impl Manager {
    /// Creates a manager for the provided extensions.
    fn with_extensions(auto_reload: bool, extensions: impl IntoIterator<Item = Extension>) -> Self {
        let mut manager = Self::base_with_context(auto_reload);
        for extension in extensions {
            manager.stage_extension(extension).unwrap();
        }

        manager
    }
}

impl LoadConflict {
    /// Creates a conflict indicating that the given extension is already loaded but its version has
    /// not changed.
    fn already_loaded(id: ExtensionID) -> Self {
        Self {
            id,
            same_version: true,
        }
    }

    /// Creates a conflict indicating that the given extension is already loaded but its version has
    /// changed.
    fn version_change(id: ExtensionID) -> Self {
        Self {
            id,
            same_version: false,
        }
    }
}
