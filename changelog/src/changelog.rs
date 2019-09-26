// Copyright 2019 Canonical Ltd. All rights reserved.  Use
// of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Simple ChangeLog lib to handle versions and logs.
//!
//! Most of the actions are wrapping commands. It would be great to
//! avoid doing that in future.

use std::fmt::{self, Display};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug)]
pub enum Error {
    // TODO(sahid): need to handle all the errors
    VersionError(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match self {
            VersionError(s) => write!(f, "unable to parse version: {}", s),
        }
    }
}

// Let's try to be a bit more concise
pub type Result<T> = std::result::Result<T, Error>;


/// Simple data structure to handle some operations arround versioning
/// [epoch:]<upstream>-[package]
pub struct Version(Option<u8>, String, String);

impl From<&str> for Version {
    fn from(value: &str) -> Self {
        Version(Self::extract_epoch(value).unwrap(),
                Self::extract_upstream(value).unwrap(),
                Self::extract_package(value).unwrap())
    }
}

impl Version {
    fn extract_epoch(value: &str) -> Result<Option<u8>> {
        let vec: Vec<&str> = value.split(':').collect();
        match vec[0].parse::<u8>() {
            Ok(v) => Ok(Some(v)),
            Err(_) => Ok(None)
        }
    }

    fn extract_upstream(value: &str) -> Result<String> {
        let vec: Vec<&str> = value.split(':').collect();
        let idx = if Self::extract_epoch(value).is_err() {
            0
        } else {
            1
        };
        match vec[idx].parse::<String>() {
            Ok(v) => Ok(v),
            Err(s) => Err(Error::VersionError(s.to_string()))
        }
    }

    fn extract_package(value: &str) -> Result<String> {
        let vec: Vec<&str> = value.split('-').collect();
        match vec[1].parse::<String>() {
            Ok(v) => Ok(v),
            Err(s) => Err(Error::VersionError(s.to_string()))
        }
    }

    pub fn incr_major(&self) -> Result<()> {
        Ok(())
    }
}

pub enum ChangeLogMessage {
    OSNewUpstreamRelease(String),
    OSNewUpstreamSnapshot(String),
    OSNewStablePointRelease(String),
    OSNewStablePointReleaseWithBug(String, String),
    NewUpstreamRelease(String),
    NewUpstreamReleaseWithBug(String, String),
}

impl Display for ChangeLogMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::ChangeLogMessage::*;
        match self {
            OSNewUpstreamRelease(s) => write!(f, "New upstream release for OpenStack {}.", s),
            OSNewUpstreamSnapshot(s) => write!(f, "New upstream snapshot for OpenStack {}.", s),
            OSNewStablePointRelease(s) => write!(f, "New stable point release for OpenStack {}.", s),
            OSNewStablePointReleaseWithBug(s, b) => {
                write!(f, "New stable point release for OpenStack {} (LP# {}).", s, b)
            }
            NewUpstreamRelease(s) => write!(f, "New upstream release {}.", s),
            NewUpstreamReleaseWithBug(s, b) => {
                write!(f, "New upstream release {} (LP# {}).", s, b)
            }
        }
    }
}

pub struct ChangeLog {
    pub workdir: PathBuf,
}

impl ChangeLog {
    pub fn new(workdir: PathBuf) -> ChangeLog {
        ChangeLog { workdir }
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
        let vec: Vec<&str> = ver.split(':').collect();
        match vec[0].parse::<u32>() {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }

    pub fn get_head_version(&self) -> Option<String> {
        let ver = self.get_head_full_version();
        let vec: Vec<&str> = ver.split(':').collect();
        if vec.len() > 1 {
            match vec[1].parse::<String>() {
                Ok(v) => return Some(v),
                Err(_) => return None,
            }
        }
        Some(ver)
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
