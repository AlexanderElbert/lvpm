# Installing lvpm

Windows 10/11 with LabVIEW 2020 or later. On Linux, lvpm is verified against
LabVIEW 2026 Q3 in NI's container image; install it with
[the Linux script](#on-linux) (from v0.3.1), or build it from source, and
see [linux.md](linux.md) for what differs there.

Every option installs two files: `lvpm` (`lvpm.exe`) and `relink-package.vi` next to
it. lvpm hands the VI to LabVIEW for `--relink`, and refuses one that did not
come with the same release, so keep them together.

## Upgrading lvpm

Rerun the installation script to install the newest release, or download and run
the newer MSI on Windows. The MSI upgrades the existing installation in place.
For a manual installation, replace both the executable and `relink-package.vi`
with the files from the same release.

Interactive commands show a notice when a newer stable release is available,
but never install it. Checks are cached daily and add at most 150 ms to a
command. See [update notifications](configuration.md#update-notifications)
for details and how to disable them.

## With the installer

Download `lvpm-x86_64-pc-windows-msvc.msi` from the
[Releases](https://github.com/labview-package-manager/lvpm/releases) page and run it. It
installs per-user to `%LOCALAPPDATA%\lvpm\bin` without elevation, adds that
folder to your user `PATH`, and can be removed under **Settings → Apps**. A
newer MSI upgrades it in place.

## With the install script

[scripts/install.ps1](../scripts/install.ps1) fetches the latest release's
MSI, verifies its SHA-256 against the digest GitHub records for it (and
against the release's checksum files), and runs it quietly: the same
per-user install as above, without the dialogs. The only things it runs are
msiexec on the verified MSI and `lvpm --version` at the end. Read it first;
it is short.

```powershell
cd $env:TEMP
irm https://raw.githubusercontent.com/labview-package-manager/lvpm/main/scripts/install.ps1 -OutFile install.ps1
.\install.ps1
```

The `cd` matters: the script is written to the current directory, and an
elevated PowerShell starts in `C:\WINDOWS\system32`, which you cannot write
to. `Access to the path 'C:\WINDOWS\system32\install.ps1' is denied` means
you are in a directory that is not yours, not that anything is wrong with
the download. lvpm installs per-user and needs no elevation.

Options: `-Version v0.2.0` pins a release, `-Portable` copies the files from
the zip instead (no uninstall entry; implied by `-InstallDir <DIR>`, and used
for releases before v0.3.1, which have no MSI), `-NoPath` leaves your `PATH`
alone, `-Sha256 <hash>` adds a hash you obtained
elsewhere as a further check, `-Stable` skips pre-releases. If PowerShell
refuses to run scripts:
`powershell -ExecutionPolicy Bypass -File .\install.ps1`.

## By hand

Download `lvpm-x86_64-pc-windows-msvc.zip` from the
[Releases](https://github.com/labview-package-manager/lvpm/releases) page, right-click it,
tick **Unblock**, extract, and put the folder on your `PATH`.

## On Linux

[scripts/install.sh](../scripts/install.sh) does for Linux x64 what
`install.ps1` does with the zip: it fetches the latest release's static
tarball (`lvpm-x86_64-unknown-linux-musl.tar.gz`), verifies its SHA-256 the
same way, and puts `lvpm` and `relink-package.vi` in `~/.local/bin`, or in
`/usr/local/bin` when run as root, as in a `Dockerfile`. It needs curl, tar,
sed, awk and `sha256sum`, and no root.

```sh
curl -fsSLO https://raw.githubusercontent.com/labview-package-manager/lvpm/main/scripts/install.sh
sh install.sh
```

Options: `--version v0.3.1`, `--stable`, `--install-dir <dir>`,
`--sha256 <hash>`, `--archive <file>` with `--sha256` for a local tarball,
and `--build-local` to build and install this checkout. It does not edit
your shell profile; it says so when the folder is not on `PATH`.

## From source

```console
cargo build --release      # target\release\lvpm.exe
cargo test
```

Toolchain setup is in [CONTRIBUTING.md](../CONTRIBUTING.md).

## Test a local release installation

From a Windows checkout with the build tools installed:

```powershell
.\scripts\install.ps1 -BuildLocal
```

This runs `cargo build --release --locked`, packages the executable,
the relink VI, README and licenses into a temporary zip, verifies its SHA-256, and uses
the portable installation path (no MSI, so no WiX needed). It replaces any existing per-user
lvpm in `%LOCALAPPDATA%\lvpm\bin` and adds that directory to your user
`PATH`. No GitHub release or cargo-dist setup is required. The version
remains the one declared in Cargo.toml; temporary packaging files are removed.
The local checksum checks the packaged bytes, not publisher authenticity.

For an isolated test without changing your installed lvpm or user PATH:

```powershell
.\scripts\install.ps1 -BuildLocal -InstallDir "$PWD\sandbox\local-installer\bin" -NoPath
.\sandbox\local-installer\bin\lvpm.exe --version
```

An existing release zip can also be installed with `-Archive <FILE>` and
`-Sha256 <EXPECTED-HASH>` instead of building it. Local modes cannot be
combined with GitHub release selection options.
