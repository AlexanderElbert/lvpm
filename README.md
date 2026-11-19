<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="docs/assets/lvpm-logo-dark.svg">
    <img alt="lvpm: open-source package manager for LabVIEW" src="docs/assets/lvpm-logo.svg" width="520">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/labview-package-manager/lvpm/actions/workflows/build.yml"><img alt="build" src="https://github.com/labview-package-manager/lvpm/actions/workflows/build.yml/badge.svg"></a>
  <a href="https://github.com/labview-package-manager/lvpm/releases"><img alt="release" src="https://img.shields.io/github/v/release/labview-package-manager/lvpm?include_prereleases&sort=semver"></a>
  <a href="https://github.com/labview-package-manager/lvpm/releases"><img alt="downloads" src="https://img.shields.io/github/downloads/labview-package-manager/lvpm/total"></a>
  <a href="#license"><img alt="license: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue"></a>
</p>

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="docs/README.md">Documentation</a> ·
  <a href="docs/roadmap.md">Roadmap</a>
</p>

**lvpm** installs LabVIEW packages from the command line. It resolves `.vip`
and `.ogp` packages by name from the public package directories, your own
hosted indexes or local folders, downloads and verifies them, and unpacks
them into a LabVIEW installation or into a project's own **[venv](docs/venvs.md)**. A project
declares what it needs in `lvpm.toml`, so every developer and every CI job
gets the same packages.

**Venvs are new to LabVIEW.** Until now every package went into the one
`vi.lib` that every project on the machine shares, so two projects needing
different versions of a library meant two LabVIEW VMs or two
machines. A venv gives each project its own packages, the way a Python
virtualenv or `node_modules` does: they live beside the project, LabVIEW
mounts them as an [LVAddons](https://labviewwiki.org/wiki/LVAddons) overlay
(LabVIEW 2024 Q1 and later), and `C:\Program Files` never changes.

```console
$ lvpm venv create --labview-version 2026
$ lvpm install
resolved 48 packages:
  delacor_lib_qmh 7.1.2.1547
  ...
+ delacor_lib_qmh 7.1.2.1547 ... 65 files
...
done. 8766 files written.
linkfix: preparing installed files (repair links and separate compiled code; no LabVIEW needed)
$ lvpm launch MyProject.lvproj
```

## Features

- **Per-project dependencies, a first for LabVIEW.** A venv keeps a
  project's packages beside it, mounted into LabVIEW as an LVAddons overlay.
  `C:\Program Files` never changes, and two projects can use two versions
  of the same library on one machine.
- **Works with the packages that exist today.** Reads VIPM's `.vip` and
  OpenG's `.ogp` formats and the public package directories NI and JKI
  host, plus any index or folder of `.vip` files you add.
- **A manifest you commit.** `lvpm.toml` lists dependencies with exact pins,
  `>=` floors or `*`, optional dependency groups (`test`, `ci`, ...), extra
  package indexes, and per-package hook and relink settings.
- **No LabVIEW needed to install.** Linkfix repairs the installed VIs' links
  and separates compiled code by editing the files directly. LabVIEW is only started for install
  hooks and an optional load-and-save relink, driven over VI Server.
- **Built for CI.** Runs unchanged in NI's Windows and Linux LabVIEW
  containers; a headless install takes seconds and starts no LabVIEW.
- **Verified downloads, shared cache.** MD5 and SHA-256 checks, and a
  content-addressed cache so a package is downloaded once per machine.

## Install

Windows 10/11 with LabVIEW 2020 or later (venvs need LabVIEW 2024 Q1 or
later). Run the MSI from the
[Releases](https://github.com/labview-package-manager/lvpm/releases) page, or
in PowerShell:

```powershell
cd $env:TEMP
irm https://raw.githubusercontent.com/labview-package-manager/lvpm/main/scripts/install.ps1 -OutFile install.ps1
.\install.ps1
```

The script verifies the MSI's SHA-256 and installs it quietly: `lvpm.exe`
and `relink-package.vi` per-user in `%LOCALAPPDATA%\lvpm\bin`, on your
`PATH`, removable under **Settings → Apps**; no elevation needed.

On Linux x64 (from v0.3.1):

```sh
curl -fsSLO https://raw.githubusercontent.com/labview-package-manager/lvpm/main/scripts/install.sh
sh install.sh
```

Script options, the portable zip, building from source and Linux are covered
in [docs/installation.md](docs/installation.md) and
[docs/linux.md](docs/linux.md).

## Quick start

**One package into a LabVIEW installation:**

```console
$ lvpm targets                              # which LabVIEW installations lvpm sees
$ lvpm search error                         # find packages in the public indexes
$ lvpm install oglib_error --labview-version 2026
$ lvpm list                                 # what lvpm has installed there
$ lvpm uninstall oglib_error
```

**A project with its own venv.** Write an `lvpm.toml` at the project root:

```toml
[project]
name = "MyProject"
labview = "2026"              # the oldest LabVIEW the project supports

[dependencies]
delacor_lib_qmh = "7.1.2.1547"    # exactly this version
oglib_error = ">=6.0.1"           # the newest version satisfying the floor

[dependencies.test]               # installed only with --with test
jki_lib_caraya = "*"
```

then:

```console
$ cd C:\Git\MyProject
$ lvpm venv create                # .lv-venv/ beside lvpm.toml, bound to LabVIEW 2026
$ lvpm install --with test        # everything lvpm.toml lists, into .lv-venv
$ lvpm launch MyProject.lvproj    # a LabVIEW that sees the venv
```

Commit `lvpm.toml`; the venv ignores itself in git. Any `lvpm` command run
inside the project folder uses its venv, with no activation step.

## Commands

| Command | What it does |
| --- | --- |
| `lvpm install [<name>[@<version>]]` | Install a package, or everything `lvpm.toml` lists |
| `lvpm uninstall <name>` / `--all` | Remove a package, optionally with its unshared dependencies |
| `lvpm list` | What lvpm has installed into the target or venv |
| `lvpm search <query>` | Search the package indexes |
| `lvpm targets` | Detected LabVIEW installations |
| `lvpm venv create` / `status` / `remove` | Manage the project's venv |
| `lvpm launch [<lvproj>]` | Check the venv against the manifest and start LabVIEW on it |
| `lvpm linkfix` / `relink` / `run-hooks` / `refresh` | Rerun one step of an install |
| `lvpm config` / `cache add` | Settings and the package cache |
| `lvpm import <file>` | Write VIPM's `vipm.toml` or a `.vipc` into `lvpm.toml` once |

All commands and flags: [docs/commands.md](docs/commands.md).

## Documentation

| | |
| --- | --- |
| [Installation](docs/installation.md) | Install script, manual install, building from source |
| [Commands](docs/commands.md) | Every command and flag |
| [The manifest (`lvpm.toml`)](docs/manifest.md) | Dependencies, groups, sources, hooks and relink |
| [VIPM's files](docs/compatibility.md) | Importing a `vipm.toml` or a `.vipc` with `lvpm import` |
| [Venvs](docs/venvs.md) | Per-project dependencies and `lvpm launch` |
| [What an install does](docs/install-process.md) | Linkfix, relinking, install hooks, VI Server |
| [Package sources and cache](docs/package-sources.md) | Public directories, own indexes, local folders |
| [Configuration](docs/configuration.md) | Settings, environment variables, logs |
| [CI in a container](docs/ci-container.md) | NI's Docker images, headless installs |
| [Linux](docs/linux.md) | What differs on Linux |
| [Package notes](docs/package-notes.md) | Commonly used packages in a venv: hooks needed, what doesn't work yet |
| [Known limitations](docs/limitations.md) | What does not work yet |
| [All documentation](docs/README.md) | Including design notes and internals |

## Why lvpm

lvpm is open source so the LabVIEW community can develop and fix the tools
it depends on. If a feature does not work the way it should, we can improve it together.

It uses the package format the OpenG Package Tools first defined, which VIPM also builds on, so the `.vip` and `.ogp` packages and the
package directories in use today work with lvpm unchanged.
[docs/architecture.md](docs/architecture.md) has more.

## Status

Working and in active development. Windows is the tested platform; Linux is
verified in NI's container image. See [known limitations](docs/limitations.md)
and the [roadmap](docs/roadmap.md).

lvpm is not affiliated with JKI or NI.

## Contributing

Toolchain setup, tests and conventions are in
[CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE),
at your option.
