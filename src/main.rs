// Copyright 2019 Canonical Ltd. All rights reserved.  Use
// of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! # Ubuntu OpenStack Package
//!
//! Collection of CLI tools helping managing Ubuntu OpenStack
//! packages.

#[macro_use]
extern crate clap;

use clap::{App, AppSettings, Arg, SubCommand};
use uosp::*;


fn get_current_dir() -> std::path::PathBuf {
    std::env::current_dir().unwrap()
}

/// Rebases a package to a new upstream version
fn rebase(name: &str, release: &str, version: &str, bugid: &str) -> Result<()> {
    println!("Rebasing {} {} to new upstream version '{}', #{}...",
             name, release, version, bugid);
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
    chg.new_release(version, &format!(
        "New upstream release {}. (LP# {})", version, bugid));

    git.debcommit()?;
    git.show()?;

    Ok(())
}

/// Creates snapshot of an upstream source and rebase the package with it.
fn snapshot(name: &str, release: &str, version: &str) -> Result<()> {
    println!("Updating package {} {} to new upstream snapshot '{}'...",
             name, release, version);

    let workdir = get_current_dir();
    let branch = Package::format_branch(release);

    let pkg = Package::clone(name, workdir.clone())?;

    let git = pkg.git.as_ref().unwrap();
    git.checkout("pristine-tar")?;
    git.checkout("upstream")?;
    git.checkout(&branch)?;

    let githash = pkg.generate_snapshot(release, version)?;
    let gitversion = pkg.version_from_githash(version, &githash);

    // The actions in a package refer always to rootdir/name/
    let archive = format!("../{}_{}.orig.tar.gz", name, gitversion);
    pkg.apply_tarball(version, &archive)?;

    let chg = &pkg.changelog;
    chg.new_release(&gitversion, &format!(
        "New upstream release {}.", version));

    git.debcommit()?;
    git.show()?;

    // Wanning that the process is not yet finished.
    // TODO(sahid): implement some sort of magic to handle deps.
    println!("");
    println!("/!\ Please consider to check (build-)deps.");
    println!("");

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

fn publish(name: &str, ppa: &str, serie: &str, fake: bool) -> Result<()> {
    println!("Backport {} to '{}', ubuntu {}, fake-time: {:?}...", name, ppa, serie, fake);

    let pkg = Package::clone(name, get_current_dir())?;
    pkg.build()?;
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
                .about("Rebase package to a new upstream release.")
                .arg(Arg::with_name("project")
                     .help("Openstack package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("release")
                     .help("Openstack release name. (e.g. stein, master).")
                     .required(true))
                .arg(Arg::with_name("version")
                     .help("Openstack version to rebase on. (e.g. 19.0.1).")
                     .required(true))
                .arg(Arg::with_name("bugid")
                     .help("Launchpad bug ID associated to the rebase.")
                     .required(true)))
        .subcommand(
            SubCommand::with_name("snapshot")
                .about("Update an Ubuntu package to a new upstream snapshot.")
                .arg(Arg::with_name("project")
                     .help("Openstack package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("release")
                     .help("Openstack release name. (e.g. stein, master).")
                     .required(true))
                .arg(Arg::with_name("version")
                     .help("The next OpenStack version. (e.g. 19.0.1~b1).")
                     .required(true)))
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
                     .help("Openstack package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("ppa")
                     .help("Launchpad PPA used. (e.g. ppa:sahid-ferdjaoui/eoan-train).")
                     .required(true))
                .arg(Arg::with_name("serie")
                     .help("Ubuntu serie used to build package. (e.g. eoan)")
                     .required(true)))
                /*
                .arg(Arg::with_name("fake")
                     .help("Use fake timestamp.")
                     .required(true)))*/
        .subcommand(
            SubCommand::with_name("clone")
                .about("Git clone OpenStack package from Ubuntu repository.")
                .arg(Arg::with_name("project")
                     .help("Openstack package name. (e.g. nova).")
                     .required(true)))
        .subcommand(
            SubCommand::with_name("pushlp")
                .about("Force push branch on a git launchpad account.")
                .arg(Arg::with_name("project")
                     .help("Openstack package name. (e.g. nova).")
                     .required(true))
                .arg(Arg::with_name("account")
                     .help("Launchpad account. (e.g. sahid-ferdjaoui).")
                     .required(true)))
        .get_matches();

    let mut ret: Result<()> = Err(Error::Fatal(
        "please consider using one of the subcommands, --help can help :)".to_string()));

    if let Some(matches) = matches.subcommand_matches("rebase") {
        ret = rebase(matches.value_of("project").unwrap(),
                     matches.value_of("release").unwrap(),
                     matches.value_of("version").unwrap(),
                     matches.value_of("bugid").unwrap());
    } else if let Some(matches) = matches.subcommand_matches("build") {
        ret = build(matches.value_of("project").unwrap());
    } else if let Some(matches) = matches.subcommand_matches("snapshot") {
        ret = snapshot(matches.value_of("project").unwrap(),
                       matches.value_of("release").unwrap(),
                       matches.value_of("version").unwrap());
    } else if let Some(matches) = matches.subcommand_matches("publish") {
        ret = publish(matches.value_of("project").unwrap(),
                      matches.value_of("ppa").unwrap(),
                      matches.value_of("serie").unwrap(),
                      /*matches.value_of("fake").unwrap()*/ true);
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
