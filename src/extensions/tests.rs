use semver::Version;

use super::conflicts::{LoadConflict, StageConflict, VersionChange};
use super::{Extension, ExtensionID, ExtensionManager as Manager, Metadata};
use crate::database::Database;
use crate::models::common::{Device, DeviceCategory, DeviceManufacturer, UniqueID};
use crate::Context;

// TODO: Add test for stage conflicts

/// Tests that any extension which has the same ID as an existing extension, but a different common
/// name, will trigger a logged warning (no matter the outcome of the load process).
#[tokio::test]
#[ignore = "not yet implemented"]
async fn name_change_log() {
    todo!()
}

/// Tests that an extension will be loaded normally if it does not conflict with an existing
/// extension, no matter what the handler mode is.
#[tokio::test]
async fn load_new_extension() {
    let db = Database::connect_with_name("load_new_extension").await;
    db.setup_tables().await.unwrap();

    // Create a basic extension
    let extension = Extension::test_single(1, 1);

    // Set the handler to standard mode and check for conflicts
    let ctx = Context::no_override();
    load_and_check_no_conflicts(&db, &ctx, &extension, true, true).await;

    // Set the handler to auto-reload mode and check for conflicts
    let ctx = Context::auto_reload();
    load_and_check_no_conflicts(&db, &ctx, &extension, true, true).await;

    // Set the handler to auto-handle mode and check for conflicts
    let ctx = Context::auto_handle();
    load_and_check_no_conflicts(&db, &ctx, &extension, true, true).await;

    db.teardown().await;
}

/// Tests that an extension which conflicts with an existing extension will be reloaded
/// automatically if the handler is in auto-reload mode.
#[tokio::test]
async fn auto_reload() {
    let db = Database::connect_with_name("auto_reload").await;
    db.setup_tables().await.unwrap();

    // Set the handler to auto-reload mode
    let ctx = Context::auto_reload();

    // Create two extensions with the same metadata, but different contents
    let (original_extension, reloaded_extension) = Extension::test_pair_same_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, &ctx, &original_extension, true, false).await;

    // Load the second extension into the database, which should replace the original extension
    let (manager, stage_conflicts) = Manager::with_extensions(&ctx, [reloaded_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(stage_conflicts.len(), 0);
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::duplicate(original_extension.metadata.id)
    );

    // Make sure the original extension was unloaded and the new version was loaded
    db.only_contains(&reloaded_extension).await;
    // Remove the extension so a case with conflicts can be tested
    db.unload_extension(&reloaded_extension.metadata.id)
        .await
        .unwrap();

    // Create two extensions with the same ID, but different versions and different contents
    let (original_extension, reloaded_extension) = Extension::test_pair_different_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, &ctx, &original_extension, true, false).await;

    // Load the second extension into the database, which should replace the original extension
    let (manager, stage_conflicts) = Manager::with_extensions(&ctx, [reloaded_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(stage_conflicts.len(), 0);
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::version_change(
            original_extension.metadata.id,
            original_extension.metadata.version.clone(),
            reloaded_extension.metadata.version.clone()
        )
    );

    // Make sured that the original extension was unloaded and the new version was loaded
    db.only_contains(&reloaded_extension).await;

    db.teardown().await;
}

/// Tests that a conflicting extension which has the same version as an existing extension will be
/// skipped if the manager is in standard or auto-handle mode.
#[tokio::test]
async fn skip_duplicate() {
    let db = Database::connect_with_name("skip_duplicate").await;
    db.setup_tables().await.unwrap();

    // Set the handler to standard mode
    let ctx = Context::no_override();

    // Create two extensions with the same metadata, but different contents
    let (original_extension, skipped_extension) = Extension::test_pair_same_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, &ctx, &original_extension, true, false).await;

    // Attempt to load the second extension into the database
    let (manager, stage_conflicts) = Manager::with_extensions(&ctx, [skipped_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(stage_conflicts.len(), 0);
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::duplicate(original_extension.metadata.id.clone())
    );

    // Make sure that the original extension was not reloaded
    db.only_contains(&original_extension).await;

    db.teardown().await;
}

/// Tests that a conflicting extension which has a lower version than an existing extension will be
/// skipped if the manager is in auto-handle mode.
#[tokio::test]
async fn skip_downgrade() {
    let db = Database::connect_with_name("skip_downgrade").await;
    db.setup_tables().await.unwrap();

    // Set the handler to auto-handle mode
    let ctx = Context::auto_handle();

    // Create two extensions with different versions and contents
    let (downgraded_extension, original_extension) = Extension::test_pair_different_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, &ctx, &original_extension, true, false).await;

    // Attempt to load the downgraded extension into the database
    let (manager, stage_conflicts) = Manager::with_extensions(&ctx, [downgraded_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(stage_conflicts.len(), 0);
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::version_change(
            original_extension.metadata.id.clone(),
            original_extension.metadata.version.clone(),
            downgraded_extension.metadata.version
        )
    );

    // Make sure that the original extension was not reloaded
    db.only_contains(&original_extension).await;

    db.teardown().await;
}

/// Tests that a conflicting extension which has a higher version than an existing extension will be
/// reloaded if the manager is in auto-handle mode.
#[tokio::test]
async fn auto_update() {
    let db = Database::connect_with_name("auto_update").await;
    db.setup_tables().await.unwrap();

    // Set the handler to auto-handle mode
    let ctx = Context::auto_handle();

    // Create two extensions with different versions and contents
    let (original_extension, updated_extension) = Extension::test_pair_different_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, &ctx, &original_extension, true, false).await;

    // Load the updated extension into the database, which should replace the original extension
    let (manager, stage_conflicts) = Manager::with_extensions(&ctx, [updated_extension.clone()]);
    let load_conflicts = manager.load_extensions(&db).await.unwrap();
    // Make sure the conflicts were correctly identified
    assert_eq!(stage_conflicts.len(), 0);
    assert_eq!(load_conflicts.len(), 1);
    assert_eq!(
        load_conflicts[0],
        LoadConflict::version_change(
            original_extension.metadata.id,
            original_extension.metadata.version,
            updated_extension.metadata.version.clone()
        )
    );

    // Make sure that the original extension was reloaded
    db.only_contains(&updated_extension).await;

    db.teardown().await;
}

/// Tests that a conflicting extension which has a different version than an existing extension will
/// cause a crash if the handler is in standard mode.
// ? What would be the "correct" way to perform an update, and how would it work?
#[tokio::test]
#[should_panic]
async fn different_version_crash() {
    let db = Database::connect_with_name("different_version_crash").await;
    db.setup_tables().await.unwrap();

    // Set the handler to standard mode
    let ctx = Context::no_override();

    // Create two extensions with different versions and contents
    let (original_extension, updated_extension) = Extension::test_pair_different_metadata();

    // Check that the original extension can be loaded without conflicts
    load_and_check_no_conflicts(&db, &ctx, &original_extension, true, false).await;

    // Attempt to load the updated extension into the database (this should panic the test)
    let (manager, _) = Manager::with_extensions(&ctx, [updated_extension]);
    manager.load_extensions(&db).await.unwrap();
}

/// Tests that the builtin extension is loaded correctly.
#[tokio::test]
async fn load_builtin_extension() {
    let db = Database::connect_with_name("load_builtin_extension").await;
    db.setup_tables().await.unwrap();

    // Add the builtin extension to the database.
    db.add_builtins().await.unwrap();

    // Make sure the builtin extension was loaded correctly.
    db.only_contains(&Extension::builtin()).await;

    db.teardown().await;
}

/// Tests that the builtin extension cannot be removed from the database.
#[tokio::test]
async fn unload_builtin_extension() {
    let db = Database::connect_with_name("unload_builtin_extension").await;
    db.setup_tables().await.unwrap();
    db.add_builtins().await.unwrap();

    // TODO: Match on error variant once custom errors are added
    assert!(db
        .unload_extension(&ExtensionID::new("builtin"))
        .await
        .is_err());

    db.teardown().await;
}

/// Tests that an extension can be loaded without generating any conflicts.
/// This test is meant to be a shortcut used by other tests, rather than a standalone test.
async fn load_and_check_no_conflicts(
    db: &Database,
    ctx: &Context,
    extension: &Extension,
    only_extension: bool,
    remove_after: bool,
) {
    // Load the extension into the database
    let (manager, stage_conflicts) = Manager::with_extensions(ctx, [extension.clone()]);
    let load_conflicts = manager.load_extensions(db).await.unwrap();
    // Make sure there were no conflicts
    assert_eq!(stage_conflicts.len(), 0);
    assert_eq!(load_conflicts.len(), 0);
    // Make sure the extension was loaded correctly
    // * The additional check for `only_contains` is not entirely necessary, but it is included to
    // * provide some extra certainty of the result.
    if only_extension {
        db.only_contains(extension).await;
    } else {
        db.contains(extension).await;
    }

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
}

impl Manager {
    /// Creates a manager for the provided extensions.
    fn with_extensions(
        ctx: &Context,
        extensions: impl IntoIterator<Item = Extension>,
    ) -> (Self, Vec<StageConflict>) {
        let mut manager = Self::base_with_context(ctx);
        let mut conflicts = Vec::new();
        for extension in extensions {
            // $ This cannot be an unwrap if it is to be tested
            let conflict = manager.stage_extension(extension).unwrap();
            if let Some(conflict) = conflict {
                conflicts.push(conflict);
            }
        }

        (manager, conflicts)
    }
}

impl LoadConflict {
    /// Creates a duplicate conflict between two copies of the same extension.
    fn duplicate(id: ExtensionID) -> Self {
        Self {
            id,
            version_change: None,
            name_change: None,
        }
    }

    /// Creates a version change conflict between two versions of the same extension.
    fn version_change(id: ExtensionID, loaded_version: Version, staged_version: Version) -> Self {
        Self {
            id,
            version_change: Some(VersionChange {
                loaded_version,
                staged_version,
            }),
            name_change: None,
        }
    }
}
