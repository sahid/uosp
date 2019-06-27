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
