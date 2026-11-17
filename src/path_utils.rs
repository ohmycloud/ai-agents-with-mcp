//! Helpers for safely resolving filesystem paths received over MCP.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, ensure};

/// Resolve a local file URI to its canonical filesystem path.
///
/// URL parsing decodes percent-escaped path segments such as spaces and
/// non-ASCII characters before the path is checked.
///
/// # Errors
///
/// Returns an error if the URI is invalid, is not a file URI, or points to a
/// path that cannot be canonicalized.
pub fn canonical_path_from_file_uri(uri: &str) -> Result<PathBuf> {
    let url = url::Url::parse(uri).with_context(|| format!("invalid file URI {uri:?}"))?;
    ensure!(url.scheme() == "file", "root URI must use the file scheme");
    let path = url
        .to_file_path()
        .map_err(|()| anyhow!("file URI cannot be converted to a local path"))?;
    path.canonicalize()
        .with_context(|| format!("file URI path does not exist: {uri:?}"))
}
