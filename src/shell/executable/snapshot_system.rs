use crate::runtime::protocol::EnvironmentSnapshot;
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};
use which::sys::RealSys;
pub(super) struct SnapshotSystem<'environment> {
    environment: &'environment EnvironmentSnapshot,
    cwd: &'environment Path,
    real: RealSys,
}
impl<'environment> SnapshotSystem<'environment> {
    pub(super) const fn new(
        environment: &'environment EnvironmentSnapshot,
        cwd: &'environment Path,
    ) -> Self {
        Self {
            environment,
            cwd,
            real: RealSys,
        }
    }
}
impl which::sys::Sys for SnapshotSystem<'_> {
    type ReadDirEntry = std::fs::DirEntry;
    type Metadata = std::fs::Metadata;
    fn is_windows(&self) -> bool {
        cfg!(windows)
    }
    fn current_dir(&self) -> io::Result<PathBuf> {
        Ok(self.cwd.to_owned())
    }
    fn home_dir(&self) -> Option<PathBuf> {
        #[cfg(windows)]
        let variable = "USERPROFILE";
        #[cfg(not(windows))]
        let variable = "HOME";
        self.environment.value(variable).map(PathBuf::from)
    }
    fn env_split_paths(&self, paths: &OsStr) -> Vec<PathBuf> {
        self.real.env_split_paths(paths)
    }
    fn env_path(&self) -> Option<OsString> {
        self.environment.value("PATH")
    }
    fn env_path_ext(&self) -> Option<OsString> {
        #[cfg(windows)]
        {
            self.environment.value("PATHEXT")
        }
        #[cfg(not(windows))]
        {
            None
        }
    }
    fn metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
        self.real.metadata(path)
    }
    fn symlink_metadata(&self, path: &Path) -> io::Result<Self::Metadata> {
        self.real.symlink_metadata(path)
    }
    fn read_dir(
        &self,
        path: &Path,
    ) -> io::Result<Box<dyn Iterator<Item = io::Result<Self::ReadDirEntry>>>> {
        self.real.read_dir(path)
    }
    fn is_valid_executable(&self, path: &Path) -> io::Result<bool> {
        self.real.is_valid_executable(path)
    }
}
