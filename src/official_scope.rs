//! Metadata-only binding guard. Never opens credential contents.
use crate::official_usage::OfficialAccountUsage;
use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::SystemTime,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthFileStamp {
    length: u64,
    modified: SystemTime,
    created: Option<SystemTime>,
    platform_identity: Option<(u64, u64, i64, i64)>,
}
impl AuthFileStamp {
    pub fn read(path: &Path) -> Result<Self> {
        let metadata = fs::symlink_metadata(path)
            .map_err(|_| anyhow!("account source metadata unavailable"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("account source must be a regular file");
        }
        #[cfg(unix)]
        let platform_identity = {
            use std::os::unix::fs::MetadataExt;
            Some((
                metadata.dev(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ))
        };
        #[cfg(not(unix))]
        let platform_identity = None;
        Ok(Self {
            length: metadata.len(),
            modified: metadata.modified()?,
            created: metadata.created().ok(),
            platform_identity,
        })
    }
}

#[derive(Clone, PartialEq, Eq)]
struct Observation {
    account: String,
    stamp: AuthFileStamp,
}
#[derive(Default)]
struct Current {
    revision: u64,
    observation: Option<Observation>,
}
#[derive(Clone)]
pub struct OfficialUsageScope {
    home: PathBuf,
    current: Arc<Mutex<Current>>,
}

pub struct BoundOfficialUsage {
    pub account: String,
    pub usage: OfficialAccountUsage,
}

impl OfficialUsageScope {
    pub fn new(home: &Path) -> Result<Self> {
        let home = if home.is_absolute() {
            home.to_owned()
        } else {
            std::env::current_dir()?.join(home)
        };
        Ok(Self {
            home,
            current: Arc::new(Mutex::new(Current::default())),
        })
    }
    pub fn observe(&self, account: Option<&str>, stamp: Option<&AuthFileStamp>) -> Result<()> {
        let next = account
            .filter(|v| !v.is_empty())
            .zip(stamp)
            .map(|(account, stamp)| Observation {
                account: account.into(),
                stamp: stamp.clone(),
            });
        let mut current = self
            .current
            .lock()
            .map_err(|_| anyhow!("official account scope unavailable"))?;
        if current.observation != next {
            current.revision = current
                .revision
                .checked_add(1)
                .ok_or_else(|| anyhow!("official scope revision overflow"))?;
            current.observation = next;
        }
        Ok(())
    }
    pub fn fetch(&self, thread: Option<&str>) -> Result<BoundOfficialUsage> {
        self.read_with(|home| crate::official_usage::fetch_official_usage_at(home, thread))
    }
    fn read_with(
        &self,
        fetch: impl FnOnce(&Path) -> Result<OfficialAccountUsage>,
    ) -> Result<BoundOfficialUsage> {
        let (revision, expected) = {
            let current = self
                .current
                .lock()
                .map_err(|_| anyhow!("official account scope unavailable"))?;
            (
                current.revision,
                current
                    .observation
                    .clone()
                    .ok_or_else(|| anyhow!("no current observed account scope"))?,
            )
        };
        let path = self.home.join("auth.json");
        if AuthFileStamp::read(&path)? != expected.stamp {
            bail!("account source changed before official read; retry after observation");
        }
        let usage = fetch(&self.home)?;
        if AuthFileStamp::read(&path)? != expected.stamp {
            bail!("account source changed during official read; result discarded");
        }
        let current = self
            .current
            .lock()
            .map_err(|_| anyhow!("official account scope unavailable"))?;
        if current.revision != revision || current.observation.as_ref() != Some(&expected) {
            bail!("account scope changed during official read; result discarded");
        }
        Ok(BoundOfficialUsage {
            account: expected.account,
            usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (tempfile::TempDir, OfficialUsageScope, AuthFileStamp) {
        let directory = tempfile::tempdir().unwrap();
        fs::write(
            directory.path().join("auth.json"),
            "synthetic opaque fixture",
        )
        .unwrap();
        let stamp = AuthFileStamp::read(&directory.path().join("auth.json")).unwrap();
        let scope = OfficialUsageScope::new(directory.path()).unwrap();
        scope.observe(Some("account-a"), Some(&stamp)).unwrap();
        (directory, scope, stamp)
    }
    #[test]
    fn known_scope_is_returned_and_unchanged_observations_do_not_invalidate_it() {
        let (directory, scope, stamp) = fixture();
        let before = fs::read(directory.path().join("auth.json")).unwrap();
        let result = scope
            .read_with(|home| {
                assert_eq!(home, directory.path());
                scope.observe(Some("account-a"), Some(&stamp))?;
                Ok(OfficialAccountUsage::default())
            })
            .unwrap();
        assert_eq!(result.account, "account-a");
        assert_eq!(
            fs::read(directory.path().join("auth.json")).unwrap(),
            before
        );
    }
    #[test]
    fn stale_source_and_missing_observation_fail_before_fetch() {
        let (directory, scope, _) = fixture();
        fs::write(directory.path().join("auth.json"), "changed").unwrap();
        assert!(
            scope
                .read_with(|_| panic!("stale context started fetching"))
                .is_err()
        );
        scope.observe(None, None).unwrap();
        assert!(
            scope
                .read_with(|_| panic!("missing context started fetching"))
                .is_err()
        );
        let legacy:crate::runtime::AccountBinding=serde_json::from_str(r#"{"account_fingerprint":"account-a","confidence":"verified","auth_generation":"old"}"#).unwrap();
        assert!(legacy.auth_file_stamp.is_none());
    }
    #[test]
    fn file_changes_scope_changes_and_aba_changes_reject_fetched_results() {
        for mode in ["file", "account", "aba", "logout"] {
            let (directory, scope, stamp) = fixture();
            let result = scope.read_with(|_| {
                match mode {
                    "file" => fs::write(directory.path().join("auth.json"), "changed size")?,
                    "account" => scope.observe(Some("account-b"), Some(&stamp))?,
                    "aba" => {
                        scope.observe(Some("account-b"), Some(&stamp))?;
                        scope.observe(Some("account-a"), Some(&stamp))?;
                    }
                    _ => scope.observe(None, None)?,
                }
                Ok(OfficialAccountUsage::default())
            });
            assert!(result.is_err(), "{mode} was accepted");
        }
    }
    #[test]
    fn metadata_capture_rejects_non_files_and_broken_links() {
        let directory = tempfile::tempdir().unwrap();
        assert!(AuthFileStamp::read(directory.path()).is_err());
        assert!(AuthFileStamp::read(&directory.path().join("missing")).is_err());
        #[cfg(unix)]
        {
            let link = directory.path().join("link");
            std::os::unix::fs::symlink(directory.path().join("missing"), &link).unwrap();
            assert!(AuthFileStamp::read(&link).is_err());
        }
    }
}
