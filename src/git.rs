// Copyright 2019 Canonical Ltd. All rights reserved.  Use
// of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

use std::fmt::{self, Display};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug)]
pub enum Error {
    // TODO(sahid): need to handle all the errors
    CloneError(String),
    Fatal(String),
}


impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;
        match self {
            CloneError(s) => write!(f, "unable to clone project {}", s),
            Fatal(s) => write!(f, "unexpected error {}", s),
        }
    }
}


impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Error::Fatal(error.to_string())
    }
}


// Let's try to be a bit more concise
pub type Result<T> = std::result::Result<T, Error>;


#[derive(Debug)]
pub struct Git {
    pub workdir: PathBuf,
}


pub trait GitClone {
    fn clone(name: &str, rootdir: PathBuf) -> Result<Git>;
}


pub struct VCSGit;


pub struct Github;


impl Git {
    pub fn new(workdir: PathBuf) -> Git {
        Git {
            workdir: workdir
        }
    }

    pub fn exists(&self) -> bool {
        self.workdir.exists()
    }
    
    pub fn checkout(&self, branch: &str) -> Result<()> {
        Command::new("git")
            .current_dir(&self.workdir)
            .arg("checkout")
            .arg(branch)
            .status()?;
        Ok(())
    }

    pub fn debcommit(&self) -> Result<()> {
        Command::new("debcommit")
            .current_dir(&self.workdir)
            .arg("-a")
            .status()?;
        Ok(())
    }

    pub fn show(&self) -> Result<()> {
        Command::new("git")
            .current_dir(&self.workdir)
            .arg("show")
            .status()?;
        Ok(())
    }

    pub fn update(&self) -> Result<()> {
        Command::new("git")
            .current_dir(&self.workdir)
            .arg("pull")
            .status()?;
        Ok(())
    }

    pub fn push(&self, url: &str) -> Result<()> {
        Command::new("git")
            .current_dir(&self.workdir)
            .arg("push")
            .arg("-f")
            .arg("--all")
            .arg(url)
            .status()?;
        Ok(())
    }

    pub fn get_hash(&self) -> Result<String> {
        let o = Command::new("git")
            .current_dir(&self.workdir)
            .arg("rev-parse")
            .arg("--short")
            .arg("HEAD")
            .output()?;
        Ok(String::from_utf8(o.stdout).unwrap().trim().to_string())
    }
}


impl GitClone for VCSGit {
    fn clone(name: &str, rootdir: PathBuf) -> Result<Git> {
        let mut workdir = rootdir.clone();
        workdir.push(name);
        let git = Git {
            workdir: workdir,
        };
        if !git.exists() {
            Command::new("mkdir").arg("-p").arg(&rootdir).status()?;
            let o = Command::new("gbp")
                .current_dir(&rootdir)
                .arg("clone")
                .arg(format!("vcsgit:{}", name))
                .status()?;
            if !o.success() {
                return Err(Error::CloneError(name.to_string()));
            }
        }
        Ok(git)
    }
}


impl GitClone for Github {
    fn clone(name: &str, rootdir: PathBuf) -> Result<Git> {
        let mut workdir = rootdir.clone();
        workdir.push(name);
        let git = Git {
            workdir: workdir,
        };
        if !git.exists() {
            Command::new("mkdir").arg("-p").arg(&rootdir).status()?;
            let o = Command::new("git")
                .current_dir(&rootdir)
                .arg("clone")
                .arg(format!("https://github.com/openstack/{}", name))
                .output()?;
            if !o.status.success() {
                return Err(Error::CloneError(name.to_string()));
            }
        }
        Ok(git)
    }
}
