# Compatibility with VIPM files

lvpm's manifest is `lvpm.toml`, and that is the only manifest lvpm reads.
VIPM's project files come in once, through `lvpm import`, which writes
them into the project's `lvpm.toml`:

- **`vipm.toml`**: VIPM 2026's TOML project file. See
  [Importing a `vipm.toml`](#importing-a-vipmtoml).
- **`.vipc`**: a VI Package Configuration, VIPM's older format. See
  [Importing a `.vipc`](#importing-a-vipc).

After the import, the project is an lvpm project. The VIPM file is not
read again, and lvpm never writes it. Using VIPM and lvpm on the same
project at the same time is not recommended: their manifests mean
different things (see [Semantic differences](#semantic-differences)), and
keeping two files in step is left to whoever edits them.

A `vipm.toml` given anywhere else is refused, with the import as the hint:

```console
$ lvpm install --manifest vipm.toml
Error: vipm.toml is VIPM's manifest; lvpm reads an lvpm.toml
hint: `lvpm import vipm.toml` writes it into the project's lvpm.toml once
```

The code is in [`src/vipm_interop.rs`](../src/vipm_interop.rs) (reading)
and `cmd_import` in [`src/main.rs`](../src/main.rs) (writing).

## The import, either format

```console
$ lvpm import vipm.toml              # a VIPM 2026 manifest
$ lvpm import deps.vipc              # a package configuration: top-level packages
$ lvpm import deps.vipc --all        # every package in the .vipc
$ lvpm import <file> --group ci      # [dependencies] go into [dependencies.ci]
$ lvpm import <file> --dry-run       # show it, write nothing
```

- **Which manifest.** The one the venv in effect is bound to; else
  `--project`'s or the nearest `lvpm.toml`; else a new `lvpm.toml` in the
  working directory. No venv is needed.
- **A venv created from a `vipm.toml`** by an earlier lvpm is not
  supported: commands that read its manifest refuse it. Remove it with
  `lvpm venv remove`, import, and create it again.
- **Versions** are written exact (`name = "1.2.3.4"`), as VIPM installs
  them. An entry the manifest already has is updated in place, in the
  group that declares it unless `--group` names one. Each change is
  reported; running the import again changes nothing.
- **`[project]` keys** (`name`, `version`, `labview`) are written only where
  the manifest has none. A different existing value is kept and reported.
- **LabVIEW.** VIPM's LabVIEW version becomes `labview`, which lvpm reads as
  a minimum: a venv may bind to that LabVIEW or a newer one. Bitness is not
  used; lvpm takes it from the LabVIEW the venv is bound to.

## Importing a `vipm.toml`

The notes are against VIPM's published documentation (docs.vipm.io, 2026
Q3). VIPM is closed source, so anything not documented there is unknown.

```console
$ lvpm import vipm.toml
import: C:\proj\vipm.toml (VIPM manifest)
  labview-version "2024" read as minimum labview = "2024"
  not used: project.labview-bitness, build.package

manifest: C:\proj\lvpm.toml (new)
  set name = "instrument-control"
  set version = "1.0.0"
  set labview = "2024"
  saved oglib_array = "6.0.1.20" [dependencies]
  saved caraya = "1.4.5.165" [dependencies.dev]
  [dev-dependencies] are the group "dev": `lvpm install --with dev`
  saved ni-visa = "24.0" [nipm.dependencies]

next: `lvpm install` installs them
```

| `vipm.toml`                                           | `lvpm.toml`                                                                                          |
| ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `[project] name`, `version`                           | The same keys, where the manifest has none.                                                          |
| `[project] labview-version = "2024"`                  | `labview = "2024"`: VIPM's pinned year becomes lvpm's minimum.                                       |
| `[project] labview-bitness`                           | Not used.                                                                                            |
| `[project] description`, `authors`, `license`, ...    | Not used.                                                                                            |
| `[dependencies] name = "1.2.3.4"`                     | The same line, exact. Empty versions and operators (`>=`, `*`, ...) are refused: VIPM documents exact versions only. |
| `[dev-dependencies]`                                  | `[dependencies.dev]`, installed with `lvpm install --with dev`, as `vipm install --dev` does.        |
| `[nipm.dependencies]` (string or `{ version, feed }`) | Copied as written. lvpm reports them and does not install them.                                     |
| `[nipm.dev-dependencies]`, `[nipm.feeds]`             | Not used.                                                                                            |
| `[build.*]`                                           | Not used. Building is out of lvpm's scope.                                                           |

Everything not used is named in the `not used` line, so nothing is dropped
silently. `--all` does not apply: a `vipm.toml` already lists only what the
project declares.

## Importing a `.vipc`

A VI Package Configuration (`.vipc`) is VIPM's older way to share a
project's packages. It is a zip holding:

- `config.xml`: the LabVIEW target and every package as `name-version`,
- each package's spec and icon,
- optionally the packages themselves (`Included` in VIPM).

### Which packages

A `.vipc` lists every package the project needs, dependencies included. By
default only the packages that nothing else in the file requires are
written, the way a project would list them by hand.

The bundled specs decide which packages those are. VIPM's `Dependency` flag
does not: it disagreed with the specs in 20 of 25 files checked.

The packages left out are listed, each with the package that pulls it in.
`lvpm install` resolves them again from their dependents' floors, so it may
pick newer versions than the file lists. `--all` pins them too.

### LabVIEW

The target's version becomes `labview`: `26.1 (64-bit)` is `"2026"`.
Targets older than LabVIEW 2009 are not named by year and are skipped.

### Packages carried in the file

Packages the `.vipc` carries go into the package cache, as
`lvpm cache add` would, so `lvpm install` finds them.

The import then checks the indexes. It ends with a note naming every such
package that no index has at that version. Only this machine's cache has
those packages, so a teammate or CI runner has to run the same import, or
the packages have to be hosted on an index in `[sources]`.

A pinned package that is neither in the file nor on any index is reported
as one the install will fail on.

### What the format looks like

Checked against 25 distinct files written by VIPM 2013 to 2026:

- every `config.xml` is `VI_Package_Configuration` version 0.6 with one
  target;
- `Included` always matches the package archive being in the zip, as
  `<name>-<version>.vip` or, for old OGPM packages, `.ogp` (with versions
  such as `1.1-1`);
- `config.xml` has no encoding declaration. It is normally UTF-8, but one
  file has the creator's name in the Windows code page, so bytes that are
  not UTF-8 are read as Latin-1;
- dependencies are in each spec's `[Dependencies] Requires=`.

## Semantic differences

What changes in meaning when a project moves from VIPM to lvpm:

- **LabVIEW version.** VIPM's `labview-version` is the team's pinned year,
  overridden per developer by `.vipm/config.toml` or `--labview-version`.
  lvpm's `labview` is a minimum; the per-developer choice is the venv
  binding (`lvpm venv create --labview-version`) or `--labview-version`, and
  nothing per-developer is written next to the manifest besides the
  gitignored venv.
- **Constraints.** VIPM records exact versions and resolves "latest" at
  `vipm add` time. lvpm lets the manifest carry floors and `*` and resolves
  them at install time; reproducibility is the job of the planned
  `lvpm.lock`, as `vipm.lock` is for VIPM. The import writes exact
  versions, so VIPM's meaning is kept; with the planned Cargo grammar it
  would write `=x`.
- **Dev dependencies.** VIPM installs `[dev-dependencies]` with
  `vipm install --dev`; lvpm installs `[dependencies.dev]` with
  `--with dev`. Neither installs them by default.
- **Installation target.** VIPM installs into the LabVIEW installation.
  lvpm installs into the project's venv by default, and refuses to install
  into the machine from inside a project without a venv (except headless).
- **NIPM.** VIPM installs `[nipm.*]` packages through NI Package Manager.
  lvpm only reports them.
- **VIPM reading an `lvpm.toml`.** Not supported. VIPM documents exact
  versions only under `[dependencies]` and reads `labview-version`, not
  `labview`; floors, `*`, group tables and `[sources]` are lvpm's own.
