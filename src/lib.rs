// Copyright 2019 Canonical Ltd. All rights reserved.  Use
// of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate git;

use std::fmt::{self, Display};
use std::path::PathBuf;
use std::process::Command;

use crate::git::{Git, GitCloneUrl};

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
        // I should refer gbp.conf
        let mut builddir = rootdir.clone();
        builddir.push("build-area");
        Command::new("mkdir").arg("-p").arg(builddir).status();

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

    /// Returns a `Package` after to have cloned its repository.
    ///
    /// By default project will be cloned using ``
    pub fn clone(name: &str, rootdir: PathBuf, kind: &str, dist: &str) -> Result<Package> {
        let mut pkg = Package::new(name, rootdir);
        let url = if dist == "ubuntu" {
            GitCloneUrl::UbuntuServerDev(name.to_string())
        } else {
            GitCloneUrl::VCSGit
        };
        pkg.git = Some(Git::new(&pkg.name, pkg.rootdir.clone(), url)?);
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
    pub fn generate_snapshot(
        &self,
        release: &str,
        version: &str,
        upstream: Option<&str>,
    ) -> Result<String> {
        let branch = Self::format_branch(release);

        // rootdir for the upstream source is './t'.
        let mut rootdir = self.rootdir.clone();
        rootdir.push("t");

        let nameup = if upstream.is_some() {
            upstream.unwrap()
        } else {
            &self.name
        };

        let gitupstream = Git::new(
            nameup,
            rootdir,
            GitCloneUrl::OpenStackUpstream(nameup.to_string()),
        )?;
        gitupstream.checkout(&branch)?;
        gitupstream.update()?;
        Command::new("pkgos-generate-snapshot")
            .current_dir(&gitupstream.workdir)
            .status()?;

        let githash = gitupstream.get_hash()?;
        let gitversion = self.version_from_githash(version, &githash);

        // The tarball generated is located in '~/tarballs', so let's
        // move it in the package rootdir.
        Command::new("/bin/sh")
            .arg("-c")
            .arg(format!(
                "mv ~/tarballs/{}_*.orig.tar.gz {}/{}_{}.orig.tar.gz",
                nameup,
                self.rootdir.to_str().unwrap(),
                nameup,
                gitversion
            ))
            .status()?;

        Ok(githash)
    }

    pub fn version_from_githash(&self, version: &str, githash: &str) -> String {
        // Really ugly...
        let o = Command::new("date")
            .current_dir(&self.workdir)
            .arg("+%Y%m%d%H")
            .output()
            .expect("unable to generate date");
        let date = String::from_utf8(o.stdout).unwrap().trim().to_string();

        format!("{}~git{}.{}", version, date, githash)
    }

    /// Publishing a package in launchpad PPA
    pub fn publish(&self, ppa: &str, serie: &str, fake: bool) -> Result<()> {
        let version = self.changelog.get_head_version().unwrap();
        // Really ugly...
        let o = Command::new("date").arg("+%Y%m%d%H%M").output()?;
        let date = String::from_utf8(o.stdout).unwrap().trim().to_string();

        //manila_9.0.0~b1~git2019061715.86823b5c-0ubuntu1.dsc
        Command::new("backportpackage")
            .current_dir(&self.rootdir)
            .arg("-S")
            .arg(format!("~ppa{}", &date))
            .arg("-u")
            .arg(ppa)
            .arg("-d")
            .arg(serie)
            // we should refer d/gbp.conf
            .arg("-y")
            .arg(format!("build-area/{}_{}.dsc", &self.name, &version))
            .status()?;
        Ok(())
    }
}

pub enum ChangeLogMessage {
    OSNewUpstreamRelease(String),
    OSNewUpstreamReleaseWithBug(String, String),
    NewUpstreamRelease(String),
    NewUpstreamReleaseWithBug(String, String),
}

impl Display for ChangeLogMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ChangeLogMessage::*;
        match self {
            OSNewUpstreamRelease(s) => write!(f, "New upstream release for OpenStack {}.", s),
            OSNewUpstreamReleaseWithBug(s, b) => {
                write!(f, "New upstream release for OpenStack {}. (LP# {}).", s, b)
            }
            NewUpstreamRelease(s) => write!(f, "New upstream release for OpenStack {}.", s),
            NewUpstreamReleaseWithBug(s, b) => {
                write!(f, "New upstream release {}. (LP# {}).", s, b)
            }
        }
    }
}

pub struct ChangeLog {
    pub workdir: PathBuf,
}

impl ChangeLog {
    pub fn new(workdir: PathBuf) -> ChangeLog {
        ChangeLog { workdir: workdir }
    }

    pub fn get_head_full_version(&self) -> String {
        let o = Command::new("dpkg-parsechangelog")
            .current_dir(&self.workdir)
            .arg("-S")
            .arg("version")
            .output()
            .expect("unable to import orig");
        String::from_utf8(o.stdout).unwrap().trim().to_string()
    }

    pub fn get_head_epoch(&self) -> Option<u32> {
        let ver = self.get_head_full_version();
        let vec: Vec<&str> = ver.split(":").collect();
        match vec[0].parse::<u32>() {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }

    pub fn get_head_version(&self) -> Option<String> {
        let ver = self.get_head_full_version();
        let vec: Vec<&str> = ver.split(":").collect();
        if vec.len() > 1 {
            match vec[1].parse::<String>() {
                Ok(v) => return Some(v),
                Err(_) => return None,
            }
        }
        return Some(ver);
    }

    pub fn new_release(&self, version: &str, message: ChangeLogMessage) {
        // TODO: case without epoch
        let newversion = match self.get_head_epoch() {
            Some(epoch) => format!("{}:{}-0ubuntu1", epoch, version),
            None => format!("{}-0ubuntu1", version),
        };
        Command::new("debchange")
            .current_dir(&self.workdir)
            .arg("--newversion")
            .arg(newversion)
            .arg(message.to_string())
            .status()
            .expect("unable to import orig");
    }
}
