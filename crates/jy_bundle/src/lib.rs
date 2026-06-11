mod api;
mod asset;
mod draft_package;
mod fs_util;
mod manifest;
mod media_cache;
mod pipeline;
mod simple_timeline_package;
mod source;
mod subtitle_style;
#[cfg(test)]
mod test_support;
mod timeline_package;

pub use api::{BundleInspection, ImportBundleOptions, ImportBundleProgress, ImportBundleSummary};

use anyhow::Result;
use camino::Utf8Path;

use crate::manifest::BundleType;
use crate::source::PreparedSource;

pub fn import_bundle(options: &ImportBundleOptions) -> Result<ImportBundleSummary> {
    import_bundle_with_progress(options, |_| {})
}

pub fn import_bundle_with_progress<F>(
    options: &ImportBundleOptions,
    progress: F,
) -> Result<ImportBundleSummary>
where
    F: FnMut(ImportBundleProgress),
{
    import_bundle_with_progress_and_cancel(options, progress, || false)
}

pub fn import_bundle_with_progress_and_cancel<F, C>(
    options: &ImportBundleOptions,
    mut progress: F,
    is_cancelled: C,
) -> Result<ImportBundleSummary>
where
    F: FnMut(ImportBundleProgress),
    C: Fn() -> bool,
{
    let prepared = PreparedSource::from_source(&options.source)?;
    let bundle = prepared.manifest()?;
    match bundle.bundle_type {
        BundleType::TimelinePackage => {
            timeline_package::import_timeline_package(options, &prepared, &bundle, &mut progress)
        }
        BundleType::DraftPackage => {
            draft_package::import_draft_package(options, &prepared, &bundle, &mut progress)
        }
        BundleType::SimpleTimelinePackage => {
            simple_timeline_package::import_simple_timeline_package(options, &prepared, &bundle)
        }
        BundleType::PipelinePackage => pipeline::import_pipeline_package(
            options,
            &prepared,
            &bundle,
            &mut progress,
            &is_cancelled,
        ),
    }
}

pub fn inspect_bundle_source(source: &Utf8Path) -> Result<BundleInspection> {
    let prepared = PreparedSource::from_source(source)?;
    let bundle = prepared.manifest()?;
    match bundle.bundle_type {
        BundleType::TimelinePackage => {
            timeline_package::inspect_timeline_package(source, &prepared, &bundle)
        }
        BundleType::DraftPackage => {
            draft_package::inspect_draft_package(source, &prepared, &bundle)
        }
        BundleType::SimpleTimelinePackage => {
            simple_timeline_package::inspect_simple_timeline_package(source, &prepared, &bundle)
        }
        BundleType::PipelinePackage => {
            pipeline::inspect_pipeline_package(source, &prepared, &bundle)
        }
    }
}
