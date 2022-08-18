#![deny(missing_docs)]

/*!
#### _A [`bevy_asset::AssetIo`] implementation for [`vach`] archives. Allowing for seamless use with an asset server_

This crate exports the [`VachAssetIo`] struct. It is an implementation of [`bevy_asset::AssetIo`] for use in defining your own asset servers in newtypes.
It in turn loads data from a [`vach`] archive, and hands it off to the [`AssetServer`](bevy_asset::AssetServer) for processing.
Check out [`AssetServer::new`](bevy_asset::AssetServer) or the [`vach`] documentation for further usage documentation.

#### Usage
```
use bevy_vach::VachAssetIo;
use bevy::prelude::*;

#[derive(Deref)]
struct VachAssetServer<T>(AssetServer<VachAssetIo<T>>);

fn main() {
    let asset_io = VachAssetIo::from_path("assets.vach").unwrap();

    App::new()
        .insert_resource(VachAssetServer(asset_io))
        .add_system(fetch_and_log)
        .run()
}

fn fetch_and_log(asset_server: Res<VachAssetServer>) {
    // Use asset_server here like any other asset server
}

```
*/

use bevy_asset::{AssetIo, AssetIoError, FileType, Metadata};
use std::{fs::File, io, path};
pub use vach::prelude::*;

/// An [`bevy_asset::AssetIo`] impl for [`vach`] formatted archives
#[derive(Debug)]
#[repr(transparent)]
pub struct VachAssetIo<T> {
    archive: Archive<T>,
}

/// Allows you to configure the [`VachAssetIo`] resource to be used in your app
pub struct AssetIoConfig {
    /// the path to load
    pub path: path::PathBuf,
    /// The [`ArchiveConfig`] to use when loading the archive
    pub archive_config: ArchiveConfig,
}

impl<T> VachAssetIo<T> {
    /// Load a [VachAssetIo] source easily from a file path
    pub fn from_path<P: AsRef<path::Path>>(
        path: P,
        mut config: Option<ArchiveConfig>,
    ) -> Result<VachAssetIo<File>, vach::prelude::InternalError> {
        let config = config.get_or_insert(Default::default());
        let source = File::open(path)?;
        let archive = Archive::with_config(source, config)?;

        Ok(VachAssetIo { archive })
    }
}

impl<T: io::Read + io::Seek> VachAssetIo<T> {
    /// Load a [VachAssetIo] source from a preconstructed [`Archive`], this allows you to use arbitrary archive sources other than files
    pub fn new(archive: Archive<T>) -> VachAssetIo<T> {
        VachAssetIo { archive }
    }
}

impl<T: io::Read + io::Seek + Sync + Send + 'static> AssetIo for VachAssetIo<T> {
    fn load_path<'a>(
        &'a self,
        path: &'a path::Path,
    ) -> bevy_asset::BoxedFuture<'a, Result<Vec<u8>, bevy_asset::AssetIoError>> {
        let block = async move {
            let str = path.to_string_lossy();
            let resource = self.archive.fetch(str);
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
        let iter = self
            .archive
            .entries()
            .into_iter()
            .map(|e| e.0)
            .filter(|id| id.starts_with(path.to_string_lossy().as_ref()))
            .map(|id| path::PathBuf::from(id))
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
