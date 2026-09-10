#[cfg(any(target_os = "macos", test))]
use super::database;
#[cfg(not(target_os = "macos"))]
use super::unsupported;
use super::{CredentialProvider, Profile, Session};
use crate::error::{Error, Kind, Result};
#[cfg(any(target_os = "macos", test))]
use std::path::Path;
use std::{fs, path::PathBuf};
#[cfg(any(target_os = "macos", test))]
use zeroize::Zeroizing;

/// Only a profile-directory reference is public; there is no cached key/session.
#[derive(Debug, Clone)]
pub struct Chrome {
    pub root: PathBuf,
}

impl Chrome {
    /// Construct the Chrome Stable macOS root without inspecting it.
    pub fn system() -> Result<Self> {
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var_os("HOME")
                .ok_or_else(|| Error::new(Kind::Unavailable, "Home directory is unavailable"))?;
            let home = PathBuf::from(home);
            if !home.is_absolute() {
                return Err(Error::new(
                    Kind::InvalidInput,
                    "Home directory must be absolute",
                ));
            }
            Ok(Self {
                root: home.join("Library/Application Support/Google/Chrome"),
            })
        }
        #[cfg(not(target_os = "macos"))]
        Err(unsupported())
    }

    /// Metadata only: enumerate immediate directory names. Do not read Local
    /// State, Preferences, Cookies, or Keychain; don't follow profile symlinks.
    pub fn discover(&self) -> Result<Vec<Profile>> {
        let entries = fs::read_dir(&self.root).map_err(|_| filesystem_error())?;
        let mut profiles = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| filesystem_error())?;
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if valid_profile(name) && entry.file_type().map_err(|_| filesystem_error())?.is_dir() {
                profiles.push(Profile {
                    directory: name.to_owned(),
                });
            }
        }
        profiles.sort_by(|a, b| a.directory.cmp(&b.directory));
        Ok(profiles)
    }

    // Injectable key reader is deliberately private. All tests supply a
    // synthetic reader; no test invokes the real OS reader, even on macOS.
    #[cfg(any(target_os = "macos", test))]
    fn load_with(
        &self,
        profile: &str,
        consent: bool,
        key_reader: impl FnOnce() -> Result<Zeroizing<Vec<u8>>>,
    ) -> Result<Session> {
        authorize(profile, consent)?;
        let path = self.cookie_path(profile)?;
        // Complete/close the DB read before requesting the broader Safe Storage
        // key. Unsupported/ambiguous storage must not prompt the Keychain.
        let pair = database::read(&path)?;
        let password = key_reader()?;
        pair.decrypt(&password)
    }

    #[cfg(any(target_os = "macos", test))]
    fn cookie_path(&self, profile: &str) -> Result<PathBuf> {
        let root = fs::canonicalize(&self.root).map_err(|_| filesystem_error())?;
        let profile_path = root.join(profile);
        require_kind(&profile_path, true)?;
        let profile_path = fs::canonicalize(profile_path).map_err(|_| filesystem_error())?;
        if profile_path.parent() != Some(root.as_path()) {
            return Err(filesystem_error());
        }
        // Both known directory layouts are recognized by metadata, not by
        // guessing a build or silently trying a different DB after failure.
        // Two present databases are ambiguous and must never be merged.
        let mut candidates = Vec::new();
        let direct = profile_path.join("Cookies");
        if exists_without_following(&direct)? {
            require_kind(&direct, false)?;
            candidates.push(direct);
        }
        let network = profile_path.join("Network");
        if exists_without_following(&network)? {
            require_kind(&network, true)?;
            let nested = network.join("Cookies");
            if exists_without_following(&nested)? {
                require_kind(&nested, false)?;
                candidates.push(nested);
            }
        }
        if candidates.len() != 1 {
            return Err(Error::new(
                Kind::Unavailable,
                "Select a profile with one supported Chrome cookie database",
            ));
        }
        let path = candidates.pop().ok_or_else(filesystem_error)?;
        let canonical = fs::canonicalize(&path).map_err(|_| filesystem_error())?;
        if canonical != path || !canonical.starts_with(&profile_path) {
            return Err(filesystem_error());
        }
        // SQLite may open sidecars itself. Reject existing symlinks and special
        // files, not just a symlink on the main DB. NOFOLLOW also protects the
        // final main-file open. Same-user concurrent directory replacement is
        // not an OS sandbox boundary; see AGENTS.md for this limitation.
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut name = path.as_os_str().to_os_string();
            name.push(suffix);
            let sidecar = PathBuf::from(name);
            if exists_without_following(&sidecar)? {
                require_kind(&sidecar, false)?;
            }
        }
        Ok(path)
    }
}

impl CredentialProvider for Chrome {
    fn load(&self, profile: &str, consent: bool) -> Result<Session> {
        authorize(profile, consent)?;
        #[cfg(target_os = "macos")]
        {
            self.load_with(profile, consent, existing_password)
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err(unsupported())
        }
    }
}

fn authorize(profile: &str, consent: bool) -> Result<()> {
    if !consent {
        return Err(Error::new(
            Kind::ConsentRequired,
            "Explicit consent is required to load Chrome session cookies",
        ));
    }
    if !valid_profile(profile) {
        return Err(Error::new(
            Kind::InvalidInput,
            "Select Default or a canonical Profile number",
        ));
    }
    Ok(())
}

fn valid_profile(profile: &str) -> bool {
    if profile == "Default" {
        return true;
    }
    let Some(number) = profile.strip_prefix("Profile ") else {
        return false;
    };
    !number.is_empty()
        && number.len() <= 10
        && number.bytes().all(|b| b.is_ascii_digit())
        && (number == "0" || !number.starts_with('0'))
}

fn filesystem_error() -> Error {
    Error::new(
        Kind::Storage,
        "Cannot safely access the selected Chrome profile",
    )
}

#[cfg(any(target_os = "macos", test))]
fn exists_without_following(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(filesystem_error()),
    }
}

#[cfg(any(target_os = "macos", test))]
fn require_kind(path: &Path, directory: bool) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| filesystem_error())?;
    let valid = if directory {
        metadata.is_dir()
    } else {
        metadata.is_file()
    };
    if metadata.file_type().is_symlink() || !valid {
        return Err(filesystem_error());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn existing_password() -> Result<Zeroizing<Vec<u8>>> {
    // In-process SecItemCopyMatching only. This API gets an existing item; no
    // create/update/reset, shell security command, or denial fallback exists.
    security_framework::passwords::get_generic_password("Chrome Safe Storage", "Chrome")
        .map(Zeroizing::new)
        .map_err(|_| {
            Error::new(
                Kind::Permission,
                "Chrome Safe Storage access was denied or unavailable",
            )
        })
}

#[cfg(test)]
mod tests;
