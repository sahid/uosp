// Copyright 2019 Canonical Ltd. All rights reserved.  Use
// of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

pub mod git;

use std::fmt::{self, Display};
use std::path::PathBuf;
use std::process::Command;

use crate::git::{Git, Github, VCSGit, GitClone};

static GIT_STABLE_BRANCH: &'static str = "stable";

#[derive(Debug)]
pub enum Error {
    VersionError(String),
    ImportError(String, String),
    ShowError(),
    BuildError(),
    Fatal(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match self {
            VersionError(s) => write!(f, "unable to download tarball {}", s),
            ImportError(p, v) => write!(f, "unable to import {} to {}", v, p),
            ShowError() => write!(f, "unable to execute git show process"),
            BuildError() => write!(f, "unable to execute buildackage process"),
            Fatal(s) => write!(f, "unexpected error {}", s),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Fatal(error.to_string())
    }
}

impl From<git::Error> for Error {
    fn from(error: git::Error) -> Self {
        Error::Fatal(error.to_string())
    }
}


// Let's try to be a bit more concise
pub type Result<T> = std::result::Result<T, Error>;

pub struct Package {
    pub name: String,
    pub rootdir: PathBuf,
    pub workdir: PathBuf,
    pub changelog: ChangeLog,
    pub git: Option<Git>,
}

impl Package {
    // TODO(sahid): This should probably return a Result<Package>
    pub fn new(name: &str, rootdir: PathBuf) -> Package {
        // TODO(sahid): Do we really need this here?
        let mut builddir = rootdir.clone();
        builddir.push("build-area");
        Command::new("mkdir").arg("-p")
            .arg(builddir)
            .status();

        let mut workdir = rootdir.clone();
        workdir.push(name);
        Package {
            name: name.to_string(),
            rootdir: rootdir,
            workdir: workdir.clone(),
            changelog: ChangeLog::new(workdir.clone()),
            git: None,
        }
    }

    /// Returns a `Package` after to have cloned its repository
    pub fn clone(name: &str, rootdir: PathBuf) -> Result<Package> {
        let mut pkg = Package::new(name, rootdir);
        pkg.git = Some(VCSGit::clone(
            &pkg.name, pkg.rootdir.clone())?);
        Ok(pkg)
    }

    /// Returns branch name based on the release.
    /// If the branch name != master returns stable/branch.
    pub fn format_branch(release: &str) -> String {
        if release == "master" {
            release.to_string()
        } else {
            format!("{}/{}", GIT_STABLE_BRANCH, release)
        }
    }

    /// Indicates whether the `workdir` for this Package exists
    pub fn exists(&self) -> bool {
        self.workdir.exists()
    }

    /// Downloads upstream release based on the `version`.  The
    /// tarball will be located at '../'.
    pub fn download_tarball(&self, version: &str) -> Result<()> {
        let o = Command::new("uscan")
            .current_dir(&self.workdir)
            .arg("--download-version")
            .arg(version)
            .arg("--rename")
            .status()?;
        if !o.success() {
            return Err(Error::VersionError(version.to_string()));
        }
        Ok(())
    }

    /// Uses gbp import-orig to apply a tarball downloaded with
    /// `download_tarball` to the package.
    pub fn apply_tarball(&self, version: &str, archive: &str) -> Result<()> {
        let o = Command::new("gbp")
            .current_dir(&self.workdir)
            .arg("import-orig")
            .arg("--no-interactive")
            .arg("--merge-mode=replace")
            .arg(archive)
            .status()?;
        if !o.success() {
            return Err(Error::ImportError(self.name.clone(), version.to_string()));
        }
        Ok(())
    }

    /// Uses gbp buildpackage to build `Package`.
    pub fn build(&self) -> Result<()> {
        Command::new("gbp")
            .current_dir(&self.workdir)
            .arg("buildpackage")
            .arg("-S")
            .arg("-sa")
            .arg("-d")
            .status()?;
        Ok(())
    }

    /// Downloads upstream release, then use pkos-generate-snapshot to
    /// create tarball. This function returns a githash as tarball
    /// identifier.
    pub fn generate_snapshot(&self, release: &str, version: &str) -> Result<String> {
        let branch = Self::format_branch(release);

        // rootdir for the upstream source is './t'.
        let mut rootdir = self.rootdir.clone();
        rootdir.push("t");

        let upstream = Github::clone(&self.name, rootdir)?;
        upstream.checkout(&branch)?;
        upstream.update()?;
        Command::new("pkgos-generate-snapshot")
            .current_dir(&upstream.workdir)
            .status()?;

        let githash = upstream.get_hash()?;
        let gitversion = self.version_from_githash(version, &githash);

        // The tarball generated is located in '~/tarballs', so let's
        // move it in the package rootdir.
        Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "mv ~/tarballs/{}_*.orig.tar.gz {}/{}_{}.orig.tar.gz",
                self.name, self.rootdir.to_str().unwrap(), self.name, gitversion))
            .status()?;

        Ok(githash)
    }

    pub fn version_from_githash(&self, version: &str, githash: &str) -> String {
        // Really ugly...
        let o = Command::new("date")
            .current_dir(&self.workdir)
            .arg("+%Y%m%d%H")
            .output().expect("unable to generate date");
        let date = String::from_utf8(o.stdout).unwrap().trim().to_string();

        format!("{}~git{}.{}", version, date, githash)
    }
}

pub struct ChangeLog {
    pub workdir: PathBuf,
}

impl ChangeLog {
    pub fn new(workdir: PathBuf) -> ChangeLog {
        ChangeLog {
            workdir: workdir,
        }
    }

    pub fn get_head_version(&self) -> String {
        let o = Command::new("dpkg-parsechangelog")
            .current_dir(&self.workdir)
            .arg("-S")
            .arg("version")
            .output()
            .expect("unable to import orig");
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    }

    pub fn get_head_epoch(&self) -> Option<u32> {
        let ver = self.get_head_version();
        let vec: Vec<&str> = ver.split(":").collect();
        match vec[0].parse::<u32>() {
            Ok(v) => Some(v),
            Err(_) => None
        }
    }

    pub fn new_release(&self, version: &str, message: &str) {
        // TODO: case without epoch
        let newversion = match self.get_head_epoch() {
            Some(epoch) => format!("{}:{}-0ubuntu1", epoch, version),
            None => format!("{}-0ubuntu1", version),
        };
        Command::new("debchange")
            .current_dir(&self.workdir)
            .arg("--newversion")
            .arg(newversion)
            .arg(message)
            .status()
            .expect("unable to import orig");
    }
}
