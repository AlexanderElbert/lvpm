# Compatibility with VIPM files

lvpm reads two of VIPM's project files:

- **`vipm.toml`** (VIPM 2026 and later) is used as it is, named with
  `--manifest`. See [manifest.md](manifest.md#compatibility-with-vipmtoml).
- **`.vipc`**, a VI Package Configuration, is imported once into an
  `lvpm.toml` with `lvpm import`, described below.

## Importing a `.vipc`

A VI Package Configuration (`.vipc`) is VIPM's older way to share a
project's packages. It is a zip holding:

- `config.xml`: the LabVIEW target and every package as `name-version`,
- each package's spec and icon,
- optionally the packages themselves (`Included` in VIPM).

`lvpm import` writes one into the project's `lvpm.toml`. After that the
`.vipc` is not read again. The code is in
[`src/vipm_interop.rs`](../src/vipm_interop.rs) (reading) and `cmd_import`
in [`src/main.rs`](../src/main.rs) (writing).

```console
$ lvpm import deps.vipc              # top-level packages, exact versions
$ lvpm import deps.vipc --all        # every package in the file, exact versions
$ lvpm import deps.vipc --group ci   # into [dependencies.ci]
$ lvpm import deps.vipc --dry-run    # show it, write nothing
```

### Which manifest

The one the venv in effect is bound to; else `--project`'s or the nearest
`lvpm.toml`; else a new `lvpm.toml` in the working directory, named after
it. No venv is needed. A venv bound to a `vipm.toml` is refused: lvpm does
not write VIPM's file.

### Which packages

A `.vipc` lists every package the project needs, dependencies included. By
default only the packages that nothing else in the file requires are
written, the way a project would list them by hand.

The bundled specs decide which packages those are. VIPM's `Dependency` flag
does not: it disagreed with the specs in 20 of 25 files checked.

The packages left out are listed, each with the package that pulls it in.
`lvpm install` resolves them again from their dependents' floors, so it may
pick newer versions than the file lists. `--all` pins them too.

### Versions

Versions are written exact (`name = "1.2.3.4"`), as VIPM installs them. An
entry the manifest already has is updated in place, in the group that
declares it unless `--group` names one. Each change is reported.

### LabVIEW

The target's version becomes `labview` when the manifest has none:
`26.1 (64-bit)` is `"2026"`, read as a minimum, as with a `vipm.toml`'s
`labview-version`. An existing `labview` is kept. Bitness is not used.
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
