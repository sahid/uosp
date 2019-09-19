// Copyright 2019 Canonical Ltd. All rights reserved.  Use
// of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

extern crate changelog;
extern crate git;

use std::fmt::{self, Display};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use changelog::ChangeLog;
use dirs::home_dir;
use git::{Git, GitCloneUrl};

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
    pub fn new(name: &str, rootdir: PathBuf) -> Result<Package> {
        // TODO(sahid): Do we really need this here?
        // I should refer gbp.conf
        let mut builddir = rootdir.clone();
        builddir.push("build-area");
        fs::create_dir_all(builddir)?;

        let mut workdir = rootdir.clone();
        workdir.push(name);
        Ok(Package {
            name: name.to_string(),
            rootdir: rootdir,
            workdir: workdir.clone(),
            changelog: ChangeLog::new(workdir.clone()),
            git: None,
        })
    }

    /// Returns a `Package` after to have cloned its repository.
    ///
    /// By default project will be cloned using ``
    pub fn clone(name: &str, rootdir: PathBuf, kind: &str, dist: &str) -> Result<Package> {
        let mut pkg = Package::new(name, rootdir)?;
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

        let home = home_dir().ok_or(
            Error::Fatal("Could not find your home directory".to_string())
        )?;
        // The tarball generated is located in '~/tarballs', so let's
        // move it in the package rootdir.
        fs::rename(
            format!("{}/tarballs/{}_*.orig.tar.gz", home.display(), nameup),
            format!("{}/{}_{}.orig.tar.gz",
                self.rootdir.to_str().unwrap(),
                nameup,
                gitversion),
            )?;
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
