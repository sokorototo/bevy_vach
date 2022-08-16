#[deny(missing_docs)]
use bevy_asset::{AssetIo, AssetIoError, FileType, Metadata};
use std::{
    fs::File,
    io,
    path::{self, PathBuf},
    sync::Arc,
};
pub use vach::prelude::*;

/// An [`bevy_asset::AssetIo`] impl for [`vach`] formatted archives
#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct VachAssetIo<T> {
    archive: Arc<Archive<T>>,
}

impl<T> VachAssetIo<T> {
    /// Load a [VachAssetIo] source easily from a file path
    pub fn from_path<P: AsRef<path::Path>>(
        path: P,
        mut config: Option<ArchiveConfig>,
    ) -> Result<VachAssetIo<File>, vach::prelude::InternalError> {
        let config = config.get_or_insert(Default::default());
        let source = File::open(path)?;
        let archive = Arc::new(Archive::with_config(source, config)?);

        Ok(VachAssetIo { archive })
    }
}

impl<T: io::Read + io::Seek> VachAssetIo<T> {
    /// Load a [VachAssetIo] source from a preconstructed [`Archive`], this allows you to use arbitrary archive sources other than files
    pub fn new(archive: Archive<T>) -> VachAssetIo<T> {
        VachAssetIo {
            archive: Arc::new(archive),
        }
    }
}

impl<T: io::Read + io::Seek + Sync + Send + 'static> AssetIo for VachAssetIo<T> {
    fn load_path<'a>(
        &'a self,
        path: &'a path::Path,
    ) -> bevy_asset::BoxedFuture<'a, Result<Vec<u8>, bevy_asset::AssetIoError>> {
        let archive = Arc::clone(&self.archive);

        let block = async move {
            let str = path.to_string_lossy();
            let resource = archive.fetch(str);
            match resource {
                Ok(res) => Ok(res.data),
                Err(err) => match err {
                    InternalError::IOError(err) => Err(AssetIoError::Io(err)),
                    InternalError::MissingResourceError(_) => {
                        Err(AssetIoError::NotFound(path.into()))
                    }
                    err => Err(AssetIoError::Io(io::Error::new(
                        io::ErrorKind::Other,
                        err.to_string(),
                    ))),
                },
            }
        };

        Box::pin(block)
    }

    fn read_directory(
        &self,
        path: &path::Path,
    ) -> Result<Box<dyn Iterator<Item = path::PathBuf>>, bevy_asset::AssetIoError> {
        let archive = Arc::clone(&self.archive);
        let iter = archive
            .entries()
            .into_iter()
            .map(|e| e.0)
            .filter(|id| id.starts_with(path.to_string_lossy().as_ref()))
            .map(|id| PathBuf::from(id))
            .collect::<Vec<_>>();

        Ok(Box::new(iter.into_iter()))
    }

    fn get_metadata(
        &self,
        path: &path::Path,
    ) -> Result<bevy_asset::Metadata, bevy_asset::AssetIoError> {
        let str = path.to_string_lossy();
        let entry = self.archive.fetch_entry(str);

        match entry {
            Some(_) => Ok(Metadata::new(FileType::File)),
            None => {
                if self
                    .archive
                    .entries()
                    .iter()
                    .map(|e| e.0)
                    .any(|e| e.starts_with(path.to_string_lossy().as_ref()))
                {
                    Ok(Metadata::new(FileType::Directory))
                } else {
                    Err(AssetIoError::NotFound(path.into()))
                }
            }
        }
    }

    // Vach archives are read only
    fn watch_path_for_changes(&self, path: &path::Path) -> Result<(), bevy_asset::AssetIoError> {
        Err(bevy_asset::AssetIoError::PathWatchError(path.into()))
    }

    fn watch_for_changes(&self) -> Result<(), bevy_asset::AssetIoError> {
        Err(bevy_asset::AssetIoError::PathWatchError("<Vach Archives are read only, so there is no need to watch for changes. Save yourself the milliseconds>".into()))
    }
}
