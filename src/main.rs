// Copyright 2019 Canonical Ltd. All rights reserved.  Use
// of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! # Ubuntu OpenStack Package
//!
//! Collection of commands helping managing Ubuntu OpenStack packages.
//!
//! Most of them are actually wrapper until to write everything in
//! pure Rust. You may need to install lot of dependencies.


#[macro_use]
extern crate clap;

use clap::{App, AppSettings, Arg, SubCommand};
use uosp::*;

const OS_MASTER: &'static str = "train";
const KIND_OPENSTACK: &'static str = "openstack";
const KIND_REGULAR: &'static str = "regular";

fn get_current_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap()
}

// https://stackoverflow.com/questions/38406793
fn uppercase_first_letter(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Rebases a package to a new upstream version
fn rebase(name: &str, version: &str, release: &str,
          bugid: Option<&str>, kind: &str, dist: &str) -> Result<()> {
    println!("Rebasing {} {} to new upstream version '{}'...",
             name, release, version);

    let workdir = get_current_dir();
    let branch = Package::format_branch(release);

    let pkg = Package::clone(name, workdir.clone())?;

    let git = pkg.git.as_ref().unwrap();
    git.checkout("pristine-tar")?;
    git.checkout("upstream")?;
    git.checkout(&branch)?;

    pkg.download_tarball(version)?;
    // The actions in a package refer always to rootdir/name/
    let archive = format!("../{}_{}.orig.tar.gz", name, version);
    pkg.apply_tarball(version, &archive)?;

    let chg = &pkg.changelog;
    // TODO(sahid): Need to move all of that in changelog, the method
    // whould be something like: chg.new_release(version, message, dist, kind)
    let msg = if kind == KIND_OPENSTACK {
        let formated_name = if release != "master" {
            uppercase_first_letter(release)
        } else {
            uppercase_first_letter(OS_MASTER)
        };
        if bugid.is_some() {
            ChangeLogMessage::OSNewUpstreamReleaseWithBug(
                formated_name, bugid.unwrap().to_string())
        } else {
            ChangeLogMessage::OSNewUpstreamRelease(formated_name)
        }
    } else {
        // Assumes KIND_REGULAR
        ChangeLogMessage::NewUpstreamRelease(version.to_string())
    };
    chg.new_release(version, msg);

    git.debcommit()?;
    git.show()?;

    Ok(())
}

/// Creates snapshot of an upstream source and rebase the package with it.
fn snapshot(name: &str, version: &str, upstream: Option<&str>) -> Result<()> {
    println!("Updating package {} to a new upstream snapshot...", name);

    let release = "master";
    let workdir = get_current_dir();
    let branch = Package::format_branch(release);
    let pkg = Package::clone(name, workdir.clone())?;

    let git = pkg.git.as_ref().unwrap();
    git.checkout("pristine-tar")?;
    git.checkout("upstream")?;
    git.checkout(&branch)?;

    let githash = pkg.generate_snapshot(release, version, upstream)?;
    let gitversion = pkg.version_from_githash(version, &githash);

    // The actions in a package refer always to rootdir/name/
    let nameup = if upstream.is_some() {
        upstream.unwrap()
    } else {
        name
    };
    let archive = format!("../{}_{}.orig.tar.gz", nameup, gitversion);
    pkg.apply_tarball(version, &archive)?;

    let msg = ChangeLogMessage::OSNewUpstreamRelease(
        uppercase_first_letter(OS_MASTER));
    let chg = &pkg.changelog;
    chg.new_release(&gitversion, msg);

    git.debcommit()?;
    git.show()?;

    // Wanning that the process is not yet finished.
    // TODO(sahid): implement some sort of magic to handle deps.
    println!("");
    println!("/!\\ Please consider to check (build-)deps.");

    Ok(())
}

/// Builds a package.
fn build(name: &str) -> Result<()> {
    println!("Building {}...", name);

    Package::new(name, get_current_dir()).build()
}

/// Clones package.
fn clone(name: &str) -> Result<()> {
    println!("Cloning package '{}'...", name);

    let pkg = Package::clone(name, get_current_dir())?;

    let git = pkg.git.as_ref().unwrap();
    git.checkout("pristine-tar")?;
    git.checkout("upstream")?;
    git.checkout("master")?;

    Ok(())
}

fn publish(name: &str, ppa: &str, serie: &str, fake: bool, build: bool) -> Result<()> {
    println!("Backport {} to '{}', ubuntu {}, fake-time: {:?}...", name, ppa, serie, fake);

    let pkg = Package::clone(name, get_current_dir())?;
    if !build {
        pkg.build()?;
    }
    pkg.publish(ppa, serie, true);

    Ok(())
}

/// Pull sources of debian packages.
fn debpull(project: &str) -> Result<()> {
    println!("Pulling debian package '{}'...", project);
    Ok(())
}

/// Git push all the source in a launchpad account.
fn pushlp(name: &str, account: &str) -> Result<()> {
    println!("Push package '{}' on lp:{}...", name, account);

    let pkg = Package::clone(name, get_current_dir())?;
    let url = format!("git+ssh://{}@git.launchpad.net/~{}/ubuntu/+source/{}",
                      account, account, name);
    pkg.git.as_ref().unwrap().push(&url)?;

    Ok(())
}

fn cli() -> std::result::Result<(), ()> {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!("\n"))
        .setting(AppSettings::GlobalVersion)
        .setting(AppSettings::SubcommandRequired)
        .setting(AppSettings::ColoredHelp)
        .subcommand(
            SubCommand::with_name("rebase")
                .about("Rebase package to a new upstream release. \n\n\
                        The process is to create point release based on the last version \
                        published upstream. Running this command will clone project name \
                        using package VCS (see: --git-url <URL> if VCS not right). Using \
                        uscan to download tarball of the proposed version and import it \
                        using gbp import-orig. Finally update the d/changelog and commit \
                        the all in git repo.")
                .arg(Arg::with_name("project")
                     .value_name("PACKAGE")
                     .help("The package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("version")
                     .value_name("VERSION")
                     .help("Openstack version to rebase on. (e.g. 19.0.1).")
                     .required(true))
                .arg(Arg::with_name("release")
                     .short("r").long("release").takes_value(true)
                     .help("Openstack release name. (e.g. stein). \
                            Default will be to consider to use the in-progress \
                            release 'master'.")
                     .default_value("master")
                     .required(false))
                .arg(Arg::with_name("bugid")
                     .short("b").long("bugid").takes_value(true)
                     .help("Launchpad bug ID associated to the rebase (e.g: 123456).")
                     .required(false))
                .arg(Arg::with_name("kind")
                     .short("k").long("kind").takes_value(true)
                     .default_value("openstack").possible_values(&["openstack", "regular"])
                     .help("Indicate what kind of package it is, this help determining \
                            version and change log message.")
                     .required(false))
                .arg(Arg::with_name("dist")
                     .short("d").long("dist").takes_value(true)
                     .default_value("ubuntu").possible_values(&["ubuntu", "debian"])
                     .help("Indicate the distribution for this package, this help determining \
                            version and change log message.")
                     .required(false)))
        .subcommand(
            SubCommand::with_name("snapshot")
                .about("Update an Ubuntu package to a new upstream snapshot.")
                .arg(Arg::with_name("project")
                     //.short("p").long("project").takes_value(true)
                     .help("Openstack package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("version")
                     //.short("v").long("version").takes_value(true)
                     .help("The next OpenStack version. (e.g. 19.0.1~b1).")
                     .required(true))
                .arg(Arg::with_name("upstream")
                     .short("u").long("upstream").takes_value(true)
                     .help("Upstream name used to grab source on github. (e.g. trove).")
                     .required(false)))
        .subcommand(
            SubCommand::with_name("build")
                .about("Build the Ubuntu package.")
                .arg(Arg::with_name("project")
                     .help("Openstack package name. (e.g. nova).")
                     .required(true)))
        .subcommand(
            SubCommand::with_name("publish")
                .about("Publish package to launchpad.")
                .arg(Arg::with_name("project")
                     //.short("p").long("project").takes_value(true)
                     .help("Openstack package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("ppa")
                     //.short("P").long("ppa").takes_value(true)
                     .help("Launchpad PPA used. (e.g. ppa:sahid-ferdjaoui/eoan-train).")
                     .required(true))
                .arg(Arg::with_name("serie")
                     //.short("s").long("serie").takes_value(true)
                     .help("Ubuntu serie used to build package. (e.g. eoan)")
                     .required(true))
                .arg(Arg::with_name("build")
                     .short("b").long("build")
                     .help("Execute package build before publishing.")
                     .required(false)))
                /*
                .arg(Arg::with_name("fake")
                     .help("Use fake timestamp.")
                     .required(true)))*/
        .subcommand(
            SubCommand::with_name("clone")
                .about("Git clone OpenStack package from Ubuntu repository.")
                .arg(Arg::with_name("project")
                     //.short("p").long("project").takes_value(true)
                     .help("Openstack package name. (e.g. nova).")
                     .required(true)))
        .subcommand(
            SubCommand::with_name("pushlp")
                .about("Force push branch on a git launchpad account.")
                .arg(Arg::with_name("project")
                     //.short("p").long("project").takes_value(true)
                     .help("Openstack package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("account")
                     //.short("a").long("account").takes_value(true)
                     .help("Launchpad account. (e.g. sahid-ferdjaoui).")
                     .required(true)))
        .get_matches();

    let mut ret: Result<()> = Err(Error::Fatal(
        "please consider using one of the subcommands, --help can help :)".to_string()));

    if let Some(matches) = matches.subcommand_matches("rebase") {
        ret = rebase(matches.value_of("project").unwrap(),
                     matches.value_of("version").unwrap(),
                     matches.value_of("release").unwrap(),
                     matches.value_of("bugid"),
                     matches.value_of("kind").unwrap(),
                     matches.value_of("dist").unwrap());
    } else if let Some(matches) = matches.subcommand_matches("build") {
        ret = build(matches.value_of("project").unwrap());
    } else if let Some(matches) = matches.subcommand_matches("snapshot") {
        ret = snapshot(matches.value_of("project").unwrap(),
                       matches.value_of("version").unwrap(),
                       matches.value_of("upstream"));
    } else if let Some(matches) = matches.subcommand_matches("publish") {
        ret = publish(matches.value_of("project").unwrap(),
                      matches.value_of("ppa").unwrap(),
                      matches.value_of("serie").unwrap(),
                      /*matches.value_of("fake").unwrap()*/ true,
                      matches.is_present("build"));
    } else if let Some(matches) = matches.subcommand_matches("clone") {
        ret = clone(matches.value_of("project").unwrap());
    } else if let Some(matches) = matches.subcommand_matches("debpull") {
        ret = debpull(matches.value_of("project").unwrap());
    } else if let Some(matches) = matches.subcommand_matches("pushlp") {
        ret = pushlp(matches.value_of("project").unwrap(),
                     matches.value_of("account").unwrap());
    }
    match ret {
        Err(e) => {
            println!("app error, {}", e);
            Err(())
        },
        Ok(_) => {
            println!("done.");
            Ok(())
        }
    }
}

fn main() {
    std::process::exit(if cli().is_ok() {0} else {1});
}
