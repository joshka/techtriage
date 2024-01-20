use super::{Extension, ExtensionID, Metadata};

/// Indicator that the manager encountered an error when loading an extension.
#[derive(Debug, PartialEq, Eq)]
pub struct LoadConflict {
    pub id: ExtensionID,
    pub same_version: bool,
}

impl LoadConflict {
    /// Checks whether a given staged extension conflicts with any of the given loaded extensions.
    /// If it does, the conflict is returned.
    // * Any staged extension can only logically have up to one conflict with a loaded
    // * extension, and vice versa, because of the following reasons:
    // * - Conflicts can only arise when a staged and a loaded extension share the same ID.
    // * - No two loaded extensions can have the same ID due to database constraints.
    // * - No two staged extensions can have the same ID because the interface prevents the same
    // *   extension from being added twice.
    pub fn new(
        staged_extension: &Extension,
        loaded_extensions: &mut Vec<Metadata>,
    ) -> Option<Self> {
        let staged_extension_metadata = &staged_extension.metadata;
        for (i, loaded_extension_metadata) in loaded_extensions.iter().enumerate() {
            if loaded_extension_metadata.id != staged_extension_metadata.id {
                continue;
            }

            let conflict = LoadConflict {
                id: loaded_extension_metadata.id.clone(),
                same_version: staged_extension_metadata.version
                    == loaded_extension_metadata.version,
            };

            // Skip the conflicting extension in subsequent conflict checks for optimization.
            loaded_extensions.remove(i);
            return Some(conflict);
        }

        None
    }

    /// Checks whether a conflict should be resolved by reloading the extension.
    pub fn should_reload(&self) -> bool {
        !self.same_version
    }
}
