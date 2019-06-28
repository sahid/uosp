# Collection of commands helping managing Ubuntu OpenStack packages.

## Important considerations

Most of them are actually wrapper until to write everything in pure
Rust. *You may need to install lot of dependencies.*

see: debian/control

```
 devscripts (>= 2.19.4),
 openstack-pkg-tools (>= 89ubuntu1),
 ubuntu-dev-tools (>= 0.166)
 apache2-dev (>= 2.4.38-2ubuntu2)
```

About the debian package, since librust-clap-dev is only available
starts to disco, we do support it from here.

## Documentation

Unfortunately not a lot so far. I would recomand to use `-help` from
the command line.

```
$ uosp help

USAGE:
    uosp <SUBCOMMAND>

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

SUBCOMMANDS:
    build       Build the Ubuntu package.
    clone       Git clone OpenStack package from Ubuntu repository.
    help        Prints this message or the help of the given subcommand(s)
    publish     Publish package to launchpad.
    pushlp      Force push branch on a git launchpad account.
    rebase      Rebase package to a new upstream release.
    snapshot    Update an Ubuntu package to a new upstream snapshot
```

## Tests/Exercises

Not a lot, there are comming time to time but feele free to help :)

## Getting Started

First be sure to have well cconfigured git, devscripts and gpg to sign
packages.

```
# Generate GPG key...

$ gpg --gen-key
...
pub   rsa3072 2019-06-28 [SC] [expires: 2021-06-27]
      4F4CC2E1100337A31E1DD544AAFF02C79E30DA5A
uid                      fakefake <fakefake@fake.com>
sub   rsa3072 2019-06-28 [E] [expires: 2021-06-27]

# Set some env vars needed by devscripts...

$ export GPGKEY=4F4CC2E1100337A31E1DD544AAFF02C79E30DA5A
$ export DEBFULLNAME="Sahid Orentino Ferdjaoui"
$ export DEBEMAIL="sahid.ferdjaoui@canonical.com"

# Git related

$ git config --global user.email "sahid.ferdjaoui@canonical.com"
$ git config --global user.name "Sahid Orentino Ferdjaoui"
```

The project is under developpement so no official relase is
available. You can build the project based on the sources and cargo or
consider using the ubuntu packages daily generated.

### From source:

```
$ git clone https://github.com/sahid/uosp.git && cd uosp
$ cargo build
$ ln -s ~/target/debug/uosp ~/bin/
```

### From PPA
```
$ sudo add-apt-repository ppa:sahid-ferdjaoui/uosp-daily
$ sudo apt-get update
$ sudo apt install uosp
```

## Contributing

The project is hosted on github but it's not clear yet if it's going
to stay here or move in launchpad. So we accept anything, patch,
pull-request, pastebin... All are welcome at any time.