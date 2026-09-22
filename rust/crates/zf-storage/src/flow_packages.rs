//! Complete immutable package acquisition and catalog publication.
mod acquire;
pub use acquire::{CapturedPackage, capture, capture_with_preconditions};
mod publication;
pub(crate) use publication::ensure_no_lifecycle_locked;
pub use publication::{
    BridgeMutation, CataloguePrecondition, LegacyRetirement, PackageConversion, PackageDeletion,
    PackagePrecondition, capture_catalogue_preconditions, convert_package, delete_package,
    recover_lifecycle,
};
pub(crate) use publication::{MARKER, recover_files_locked};
pub use publication::{PackageWrite, PendingPackage, begin, recover};

pub(crate) use publication::{
    begin_checked, guard_catalogues, inspect_catalogue_preconditions, recover_import,
};
