# Changelog

What changed in each lvpm release. Downloads are on the
[releases page](https://github.com/labview-package-manager/lvpm/releases);
installing is described in [docs/installation.md](docs/installation.md).

## 0.3.1 - 2026-11-19

First stable 0.3 release. It lists everything since 0.2.1, the last stable
release; the release candidates 0.3.1-rc.1 to rc.3 led up to it.

### Upgrading from 0.2.x

Read this first: several things work differently.

- **Venvs live in `.lv-venv/`.** A `.project/` venv made by 0.2.x is no longer
  used. Delete it, then run `lvpm venv create` and `lvpm install`. The
  `venv.dir` setting chooses another folder name.
- **Install records need a dependency list.** Packages installed by 0.2.x are
  refused by `lvpm uninstall` and `lvpm list`. Uninstall them with the lvpm
  that installed them before upgrading, or remove and recreate the venv.
- **`--repo` is gone.** A folder of `.vip` files is a per-user setting,
  `lvpm config set sources.local <dir>`, or `lvpm cache add <path>` imports
  archives into the shared cache. A local path under `[sources]` in
  `lvpm.toml` is refused, since it rarely means the same thing on another
  machine.
- **`vipm.toml` comes in through `lvpm import`.** `install --manifest
  vipm.toml` no longer reads it; `lvpm import vipm.toml` writes its packages
  into `lvpm.toml` once. A venv created from a `vipm.toml` is moved onto the
  new `lvpm.toml` by that import, with nothing to reinstall.
- **Install hooks follow a policy.** A venv runs no package's hooks unless
  `lvpm.toml` says `hooks = "run"` for it; a LabVIEW installation runs them all
  unless it says `hooks = "skip"`; a headless machine (`LV_RTE_HEADLESS`) runs
  only those marked `"run"`. `--hooks` and `--no-hooks` override.
- **LabVIEW's relink pass is opt-in.** Installed files are prepared by linkfix
  instead, without LabVIEW. `--relink`, or `relink = true` for a package in
  `lvpm.toml`, still loads and saves in LabVIEW.
- **`lvpm.toml` is read more strictly.** A version that is not a version
  (`"latest"`) is an error, as is a dependency table with a misspelt key
  (`hook = "run"`). Development dependencies are a group,
  `[dependencies.dev]`; `[dev-dependencies]` is not read.
- **The repository moved** to `labview-package-manager/lvpm`. The install
  script's address changed with it.

### New

- **Linux.** A static x64 build for LabVIEW on Linux, verified in NI's
  LabVIEW 2026 Q3 container image, with an install script
  ([docs/linux.md](docs/linux.md)).
- **A Windows installer.** A per-user MSI, on `PATH`, no elevation; the
  install script verifies its SHA-256 and installs it.
- **Venvs, reworked.** Each package is its own LVAddons addon, so a venv
  overlays the LabVIEW installation without changing it. Each venv has its
  own `labview.ini`, compiled-object cache, `LabVIEW Data` and temp folder,
  and lvpm never starts a second LabVIEW on it. Any command run inside the
  project uses its venv; `--project` and `--global` choose otherwise.
- **`lvpm launch` checks the venv first.** It refuses to start when a package
  the manifest lists is missing, at the wrong version, missing files, or owes
  a relink or PostInstall hook, and names the command that fixes each.
  `--no-check` starts anyway.
- **A shadowing warning.** After an install, at launch and in
  `lvpm venv status`, lvpm names every venv package that the LabVIEW
  installation also has, since LabVIEW loads that copy instead.
- **Linkfix.** Installed VIs get their dependency links repaired and their
  compiled code separated by editing the files directly, with no LabVIEW
  running. It also restores the Call Library wildcards VI Package Builder
  removes. `lvpm linkfix` runs it on an existing tree, with `--dry-run` and a
  per-file `--report`.
- **`lvpm.toml` grows.** Dependency groups (`[dependencies.test]`, chosen
  with `--with`, `--without` and `--only`, and `include-groups`), and
  per-package `hooks` and `relink` settings.
- **Install and uninstall keep `lvpm.toml` in step.** In a venv,
  `lvpm install <pkg>` adds the package to the manifest (`--group` picks the
  group) and `lvpm uninstall <pkg>` takes it out. `--no-save` leaves the
  manifest alone.
- **`lvpm import`** writes a VIPM `vipm.toml` or `.vipc` into `lvpm.toml`. The
  packages a `.vipc` carries go into the package cache, so they install
  without an index.
- **Dependency-aware uninstall.** `lvpm uninstall` refuses a package another
  installed package needs (`--force` overrides), offers to remove the
  dependencies nothing else needs (`--with-deps`, `--keep-deps` for scripts),
  and keeps the ones the manifest declares.
- **A shared package cache**, indexed in SQLite: each archive is downloaded
  once per machine and checked against its MD5 and SHA-256.
  `lvpm cache add` imports archives you already have. Downloads are retried,
  bounded in time, and all fetched before the first file is written.
- **Logs.** One continuous log per venv and `lvpm.log` for everything else,
  in one folder; `--log-level` (or `LVPM_LOG`) sets how much they hold.
- **Settings.** `lvpm config get` and `set`, or `LVPM_CONFIG_*` environment
  variables: the cache folder, local package folders, the venv folder name
  and the log folder.
- **Shared palette categories in a venv.** Packages that add to the same
  palette category now all show up, in the Functions and Controls palettes
  (`<menus>`) and in the Addons palette (`.mnu` files under
  `<vi.lib>\addons`, such as the HSE packages' shared `HSE` category). lvpm
  keeps each package's palette files apart and merges them into one reserved
  `venv-palettes` addon on install, uninstall and migration; `lvpm launch`
  refuses a stale merged tree, and one `lvpm install` rebuilds it (#26). When
  packages ship different `dir.mnu` files for one category, the install says
  whose it kept.
- **The venv's name in LabVIEW's window titles.** `lvpm launch` starts a
  hidden watcher that tags the venv LabVIEW's own windows with the project's
  name, e.g. `Main.vi Front Panel on MyProject.lvproj/My Computer [MyProject]`.
  The new `venv.title` setting puts it at the `end` (default), `start`, or
  turns it `off`. Windows only.
- **The venv's own icon and taskbar button.** Each venv gets `venv.ico`, the
  bound LabVIEW's icon with a coloured badge chosen from the project's name,
  and its own taskbar group, so a pinned button relaunches the venv through
  `lvpm launch`.
- **`lvpm venv shortcut [<lvproj>]`** writes a Windows shortcut that launches
  the venv (`--desktop` or `--out PATH`). It runs `lvpm launch`, so the venv's
  TEMP, the manifest check and the one-LabVIEW-per-venv guard all apply.
- An install reports how long it and each of its phases took.

### Fixed

- The resolver holds each package to the highest version floor anything asks
  for, installs packages after the ones they need, and keeps a manifest's own
  pin, with a warning, when a dependency asks for more.
- An unreadable install record of another package no longer stops an install
  part-way.
- Installing an older version than the one installed no longer writes that
  older version into `lvpm.toml`.
- `lvpm list` and `lvpm run-hooks --all` follow the manifest's `hooks`
  settings for a LabVIEW installation too.
- `lvpm uninstall <pkg> --with-deps` takes the package out of `lvpm.toml` even
  when one of its dependencies fails to uninstall.
- Commands outside a venv write their log as they go, so an interrupted run
  keeps what it logged.
- `lvpm venv status` in a project without a venv says so instead of failing.
- `lvpm venv create` no longer fails with "stream did not contain valid UTF-8"
  on a `LabVIEW.ini` that is not UTF-8. LabVIEW writes the file in the ANSI
  code page, and LabVIEW 2025 was seen writing stray bytes into key names;
  the venv's copy keeps them byte for byte.
- Security updates of the `zip` and TLS libraries.

### Known limitations

- `Replace Mode = If Newer` leaves a file that is already on disk alone.
- No dependency conflict resolution: each package gets the newest version
  meeting the highest floor anything asks for.
- A hook VI's own `error out` is not read back.
- In a venv, `PreInstall` hooks cannot run, and file groups aimed at machine
  locations (`<OS ...>`, `<temp>`) are skipped.
- Linux is verified in NI's container image only, and does not yet honour a
  package's `Exclusive_OS`.

The full list is in [docs/limitations.md](docs/limitations.md).

## 0.3.1-rc.3 - 2026-11-19

Pre-release of 0.3.1; its changes are listed under 0.3.1.

## 0.3.1-rc.2 - 2026-11-15

Pre-release of 0.3.1; its changes are listed under 0.3.1.

## 0.3.1-rc.1 - 2026-11-13

Pre-release of 0.3.1; its changes are listed under 0.3.1.

## 0.2.1 - 2026-09-23

- **Relinking works on LabVIEW older than 2026.** Every package failed to
  relink on LabVIEW 2025 and earlier with VI Server error 122: lvpm stamped
  the data it sent as LabVIEW 2026, which an older LabVIEW rejects. It is now
  stamped LabVIEW 2020, the oldest release lvpm supports.
- The install script prints the installed version again, parses on Windows
  PowerShell 5.1, and finds the latest release when no version is given.

## 0.2.0 - 2026-09-23

- **Per-project dependencies (venvs).** `lvpm venv create` makes a venv
  mounted through LVAddons, so a project's packages live beside it instead of
  in the LabVIEW installation; `lvpm launch` starts a LabVIEW that sees it.
- **`lvpm.toml`**, lvpm's own manifest, with `[sources]` and version
  constraints; `lvpm install` with no arguments installs everything it lists.
- **Headless LabVIEW** for containers and CI: relink and hooks are off by
  default in a headless run, and `--relink` and `--hooks` turn them on.
- Installing over an installed package upgrades it; a downgrade needs
  `--allow-downgrade`.
- `lvpm uninstall --all`, `lvpm refresh`, `lvpm start`, `lvpm run-hooks` and
  `lvpm app-probe`.
- Palettes and menus are refreshed after install, uninstall and hooks.
- An install script that verifies the download's SHA-256.
- A number inside a package name is no longer read as its version.

## 0.1.0-alpha.1 - 2026-08-26

First alpha: `lvpm install`, `uninstall`, `list`, `search`, `targets` and
`relink`, resolving packages from the public repositories, verifying MD5, and
unpacking into a LabVIEW installation or a `--prefix` sandbox. Install and
uninstall hooks run through VI Server, and installed packages are relinked in
LabVIEW. Windows only.
