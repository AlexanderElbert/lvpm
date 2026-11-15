# The project manifest (`lvpm.toml`)

`lvpm.toml` declares what a LabVIEW project depends on: the packages it
needs, the minimum LabVIEW it targets, and any hosted package indexes beyond
the public ones. It is the file you commit; the venv (`.lv-venv/` by default)
and the shared package cache are derived from it and are not.

This document is the reference for the format as implemented in
[`src/project.rs`](../src/project.rs), and how commands find and use it.
VIPM's `vipm.toml` and `.vipc` files come in through `lvpm import`; see
[compatibility.md](compatibility.md). Planned extensions are listed
at the end and in
[roadmap.md](roadmap.md#the-manifest-and-the-lockfile---wip).

## A complete example

```toml
[project]
name = "YourProject"
version = "0.1.0"
labview = "2025"                  # the oldest LabVIEW the project is meant for

[sources]                         # repositories beyond the public indexes
mirror = "http://host:8090/files" # a hosted index.vipr folder
defaults = true                   # false: only the sources listed here

[dependencies]
delacor_lib_qmh = "7.1.2.1547"    # exactly this version
oglib_error = ">=6.0.1"           # the newest version satisfying the floor

[dependencies.test]                # installed only with --with test
jki_lib_caraya = "*"              # the newest version

[nipm.dependencies]               # NI Package Manager packages: reported, not installed
ni-daqmx-labview-support = "26.0"
```

Every table is optional. A file containing only `[project]` is a valid
manifest with no dependencies. When no manifest exists, `lvpm venv create`
writes a skeleton: `name` from the directory, `labview` set to the year the
venv was bound to, and an empty `[dependencies]`.

## Finding the manifest

A project is wherever its manifest is, the way cargo finds `Cargo.toml`. The
manifest a command uses is chosen in this order, first match wins:

1. `install --manifest <FILE>` / `venv create --manifest <FILE>`: that file,
   whatever its name.
2. The repository of the venv in effect: `<repo>/lvpm.toml`, if it exists.
3. `--project <DIR>`: `<DIR>/lvpm.toml`, if it exists.
4. The nearest `lvpm.toml` at or above the working directory.

No manifest is fine: `lvpm install <name>` works without a project. When a
manifest is found, its `[sources]` apply to every command that reads indexes
(`search`, single-package `install`, full `install`), not only to a
manifest-driven install.

Only the file name `lvpm.toml` is discovered. `--manifest` takes a file in
lvpm.toml's format under any name; a `vipm.toml` is refused there, with
`lvpm import vipm.toml` as the hint (see [VIPM's files](#vipms-files)).

`--manifest` chooses *what* to install, not *where*. The venv an `install`
writes into is still the nearest one at or above the working directory (or
`--project`'s), wherever the named manifest lives; `venv create --manifest`
puts the venv beside the manifest, so running from that folder lines the two
up.

## `[project]`

| Key       | Type   | Used for                                                                                       |
| --------- | ------ | ---------------------------------------------------------------------------------------------- |
| `name`    | string | Shown in the install banner. Optional.                                                         |
| `version` | string | Recorded, not acted on. Optional.                                                              |
| `labview` | string | The **minimum** LabVIEW year the project supports, e.g. `"2025"`. Optional but recommended. |

`labview` is a floor, not a pin:

- `lvpm venv create` binds the venv to `--labview-version` when given, else to
  `labview`. A version newer than `labview` is accepted; an older one is
  refused. With neither, `venv create` asks for one.
- On a headless machine (`LV_RTE_HEADLESS` set) with no venv beside the
  manifest, `lvpm install` targets the LabVIEW `labview` names, or a newer
  installed one, instead of refusing (see
  [CI in a container](ci-container.md#headless-mode)).
- A global install still takes its target from `--labview-version`.

## `[sources]`

Hosted package indexes added to the search, in file order, ahead of the
public defaults.

| Key        | Type    | Meaning                                                                           |
| ---------- | ------- | --------------------------------------------------------------------------------- |
| `<name>`   | string  | An `http://` or `https://` folder holding an `index.vipr`. The name is a label.   |
| `defaults` | boolean | `true` (default): also use the public indexes. `false`: only the sources listed. |

The public defaults are NI's `download.ni.com/.../index.vipr` and JKI's
`jkisoft.ogpd`. A path on disk is refused here rather than silently resolving
nothing: local `.vip` directories are a per-user setting
(`lvpm config set sources.local`) and imported packages live in the shared
cache (`lvpm cache add`), because a filesystem path rarely means the same
thing on a teammate's machine or a CI runner.

## `[dependencies]`

One line per package: the package name as the index knows it (the `.vip`
basename without version, e.g. `oglib_error`), and a quoted constraint.

| Constraint      | Meaning                                                |
| --------------- | ------------------------------------------------------ |
| `"7.1.2.1547"`  | Exactly this version; no other is accepted.            |
| `">=6.0.1"`     | The newest available version not below the floor.     |
| `"*"` or `""`   | The newest available version.                          |

Rules:

- The version must be quoted. `oglib_array = 6.0` is a TOML float and is
  rejected, as is an array or any other non-string value. The one other
  form is a dependency table (below).
- Versions compare component by component the way VIPM numbers them
  (`major.minor.patch.build`); odd but real forms such as `1.0.1-1` are
  accepted as written. A value that is not a version, such as `"latest"` or
  `"v1.0"`, is rejected with an error naming the dependency.
- Other operators (`<`, `=`, `~`, `^`, ranges) are rejected with an error
  naming the dependency, rather than misread.
- The install plan lists dependencies before the packages that need them.

### Dependency tables: `hooks` and `relink`

A dependency can be a table instead of a version string, to set whether
the package's install hooks run and whether LabVIEW relinks it:

```toml
[dependencies]
my_lib_with_setup = { version = "2.1.0.4", hooks = "run" }
wiresmith_technology_lib_g_cli = { version = "3.0.1.98", hooks = "skip" }
sklein_lib_webview2 = { version = "2.1.1.2", relink = true }
```

| Key       | Meaning                                                                 |
| --------- | ----------------------------------------------------------------------- |
| `version` | Required. The constraint, exactly as in the string form.               |
| `hooks`   | `"run"` or `"skip"`. Unset: a venv runs none of the package's hooks, a LabVIEW installation runs them all, a headless machine none. |
| `relink`  | `true`: after linkfix, the install loads and saves the package's VIs in LabVIEW, the pass `lvpm relink` runs. Unset or `false`: linkfix only. |

Since a venv and a headless machine already skip hooks when `hooks` is
unset, `hooks = "skip"` only changes anything when the manifest installs
into a LabVIEW installation on a desktop (`lvpm install --global`). There it
opts a package out of hooks that would otherwise run, as for the G-CLI line
above. `hooks = "run"` is the opt-in that matters in a venv or headless.

`--no-hooks` and `--hooks` on `lvpm install` override `hooks`; `--relink`
relinks every package in place of linkfix, whatever `relink` says. To set
either key for a package that is only a dependency of another, declare it too.
Both settings also hold in groups. Two groups that set one differently for
the same package are an error.

`relink = true` is for packages whose VIs only LabVIEW can repair. A .NET
invoke node stores the index of its method in the assembly. A package built
against a different build of the assembly than the one it ships (as
`sklein_lib_webview2` 2.1.1.2 is) loads with broken nodes. Linkfix only
rewrites link paths, so it cannot fix them; loading and saving in LabVIEW
looks the methods up by name again. VIPM does this load and save for every
package by default (its "mass compile after install"), so such a package
works there by accident.

A table under `[dependencies]` is read as a dependency when its keys are
`version` and, optionally, `hooks` and `relink`. Any other table is a
[group](#named-dependency-groups). So a group cannot hold only packages named
`version`, `hooks` and `relink`.

### How the manifest resolves

`lvpm install` with no package name installs the selected groups (only
`[dependencies]` by default, see
[named dependency groups](#named-dependency-groups)) as one plan with one relink
pass:

- Each root asks for what its constraint says. A dependency declared by a
  package's own spec is always treated as a floor.
- Resolution is depth-first from all roots at once, with no backtracking.
  Each package is held to the highest floor anything asks for: the walk
  repeats until no pass finds a higher one. A root's own constraint is never
  replaced by a dependency's floor; a pin below one is kept, with a warning
  naming the dependent. Pin a version exactly in the manifest when a
  transitive floor would otherwise pick something you do not want.
- The plan installs every package after the packages it requires; roots keep
  their manifest order where nothing forces otherwise.
- A transitive dependency that is already installed at a version satisfying
  its floor is left alone. A root always resolves to what the index says, and
  the installer then decides whether that is a no-op, an upgrade, or a refused
  downgrade (`--allow-downgrade` permits it).
- Every root that cannot be found is collected and reported together, so one
  typo does not hide the others. A missing transitive dependency is reported
  and skipped.
- An empty selection is an error for a manifest-driven install.

`lvpm install <name>[@<version>]` inside a project installs that one package
using the project's `[sources]`. On a venv it is also written into the
venv's manifest: `name@version` saves what was given (`name@>=6.0` too),
a bare name the version installed, exact. An entry that exists keeps its
group, key spelling, `hooks` and `relink`; only its version changes.
`--group <g>` saves into `[dependencies.<g>]` (created when missing; `main`
is `[dependencies]`). `lvpm uninstall <name>` on a venv removes the entry
from every group declaring it, after the uninstall succeeded; what only that
package needed may then go too. `--no-save` leaves the manifest alone for
either command, and `uninstall --all` never edits it. The edit keeps
comments and formatting (`toml_edit`).

`lvpm install` is additive: it never removes a package that is installed but
no longer listed. Use `lvpm uninstall <name>`, or `lvpm uninstall --all`
followed by `lvpm install`, until `lvpm sync` exists (see [To do](#to-do)).
Uninstall can also remove unshared dependencies after confirmation; use
`--with-deps` or `--keep-deps` for non-interactive use. It refuses a
package other installed packages require (`--force` overrides) and keeps
shared dependencies. A dependency the project needs only through the package
being removed is not protected by the manifest; it goes, or not, like any
other unshared dependency. Old installation records without
dependency information are rejected. Recreate affected venvs and reinstall
their packages, or uninstall affected global packages with the previous lvpm
version before reinstalling them.
In a venv, packages declared in any dependency group and their installed
transitive dependencies are kept during automatic dependency removal.
Explicitly uninstalling a declared package warns and leaves the manifest
unchanged. `uninstall --all` still removes every package.
Venvs remember the manifest used at creation, including one named with
`--manifest`. Venvs whose bindings do not identify their manifest must be
removed and recreated, as must a venv an earlier lvpm created from a
`vipm.toml`.

## Named dependency groups

Named sets of packages the project needs only sometimes: test frameworks,
CI tooling, build tools. They belong to the project and never become package
metadata (as in [PEP 735](https://peps.python.org/pep-0735/)). Each group is
a nested table under `[dependencies]` with the shape of `[dependencies]`:

```toml
[dependencies]                    # the group "main"
delacor_lib_qmh = "7.1.2.1547"

[dependencies.test]
jki_lib_caraya = "*"
oglib_error = ">=6.0"

[dependencies.ci]
include-groups = ["test"]         # everything in test, plus:
ni_vi_analyzer_tests = "1.2.0.3"

[dependencies.build]
vipm_package_builder = "*"
```

- **Same syntax as `[dependencies]`.** A group line takes the same
  constraints with the same meaning, so a line can move between tables
  unchanged.
- **`include-groups`** is the one reserved key: an array of group names whose
  packages this group takes too. An unknown name is an error when the
  manifest is read; a cycle is an error naming it (`a -> b -> a`) when the
  group is selected.
- **`main`** is the name of `[dependencies]` on the command line, so no group
  may be called that.
- A package named in more than one selected table must ask for the same
  version in each; otherwise the error names both tables.

Only `main` is installed by default. The `install` flags choose more, each
taking a comma-separated list:

| Command                          | Installs                                      |
| -------------------------------- | --------------------------------------------- |
| `lvpm install`                   | main                                          |
| `lvpm install --with test`       | main + test                                   |
| `lvpm install --with ci`         | main + test + ci (`ci` includes `test`)       |
| `lvpm install --with ci --without test` | main + ci                              |
| `lvpm install --only test`       | test only, no `[dependencies]`                |
| `lvpm install --only main,build` | main + build                                  |
| `lvpm install --without main --with build` | build only                          |

`--without` applies after includes are expanded. `--only` replaces the
default and cannot be combined with `--with` or `--without`; none of the
three apply to a single-package `lvpm install <name>`. Each flag may also be
repeated (`--with test --with build` is `--with test,build`); names are
trimmed, empty entries ignored, and group names are case-sensitive. The
install banner names the groups in effect and lists the ones not selected.

## `[nipm.dependencies]`

NI Package Manager packages the project needs (drivers, toolkits). lvpm
reads only the names, reports them after resolving (`note: the manifest also
lists N [nipm.dependencies]`), and does not install them; install them with
NIPM. Values may be a version string or a table; neither is interpreted yet.

## Unknown keys and tables

Keys and tables lvpm does not know are ignored, not rejected. This keeps a
manifest readable by older lvpm builds as the format grows, and lets one file
carry another tool's sections (see below). The flip side is that a misspelt
key such as `labveiw` is silently ignored; check the install banner, which
echoes the manifest path, project name, groups and `labview` minimum it read.
This includes `[dev-dependencies]` in an `lvpm.toml`: VIPM's table, which
`lvpm import` turns into `[dependencies.dev]`.

## VIPM's files

lvpm reads `lvpm.toml` only. A VIPM 2026 `vipm.toml` or an older `.vipc`
package configuration is written into the project's `lvpm.toml` once, with
`lvpm import`:

```console
$ lvpm import vipm.toml     # [dependencies], [dev-dependencies] as group dev, labview-version as labview
$ lvpm import deps.vipc     # its top-level packages; packages carried in it go into the cache
```

How each field maps, what is not used, and how the two tools differ in
meaning: [compatibility.md](compatibility.md). Using VIPM and lvpm on the
same project at the same time is not recommended.

## To do

Tracked in [roadmap.md](roadmap.md#the-manifest-and-the-lockfile---wip):

- **`lvpm.lock`**, next to `lvpm.toml`: the resolved closure (name, version,
  source, checksum) so `lvpm install` reproduces a venv exactly and only
  `lvpm update` re-resolves. The format of `vipm.lock` is a reference point.
- **`lvpm sync`**, after `lvpm.lock`: make the venv match the lock exactly.
  Install what is missing or out of date, as `install` does, and uninstall
  every package the lock does not list (running its uninstall hooks).
  `--dry-run` shows both lists. Not the same as VIPM's `vipm sync`, which
  updates `vipm.toml` from a `.lvproj`.
  - Why after the lock: the resolver does not produce the full set of
    needed packages today. A dependency already installed at a satisfying
    version is not followed further, so its own dependencies would look
    unneeded and be removed. The lock records the whole closure.
  - Pieces already there: `install::list_installed` and the uninstall
    sequence (`uninstall_one`: PreUninstall, recorded files, PostUninstall,
    one palette refresh). Still inline in `cmd_install`: the resolver and
    the install sequence, to be extracted so `install` and `sync` share them.
  - Proposed scope: venvs and `--prefix` only, refused with `--global`;
    remove extras first, once resolution has succeeded.
- ~~**`lvpm add` / `lvpm remove`**~~: done the npm way instead, as
  `install <name>` and `uninstall <name>` editing the manifest on a venv
  (see [How the manifest resolves](#how-the-manifest-resolves)).
- **Future constraint grammar (Cargo)**, one for every dependency table:

  | Written           | Meaning                                    | Example range                       |
  | ----------------- | ------------------------------------------ | ----------------------------------- |
  | `"7.1.2.1547"`    | caret (bare), same as `^`                  | `>=7.1.2.1547, <8`                  |
  | `"^7.1.2.1547"`   | caret: leftmost non-zero part stays fixed  | `^0.3.1` is `>=0.3.1, <0.4`         |
  | `"~7.1.2"`        | tilde: only the last given part may change | `>=7.1.2, <7.2`                     |
  | `"=7.1.2.1547"`   | exact                                      |                                     |
  | `">=6.0.1"`       | floor                                      |                                     |
  | `"*"`             | newest                                     |                                     |

  - Versions have up to four parts (`major.minor.patch.build`); caret and
    tilde apply to whichever parts are written. Year-numbered packages
    (`2018.0.7.45`) get "same year" from a caret, so pin them with `=` when
    that is too loose.
  - **Breaking change**: today a bare version is exact. Existing manifests
    keep their meaning only if their pins are rewritten as `=x`; lvpm should
    say so once, when it reads a bare version that a caret would widen.
  - Python's `==` is not accepted: `=` is the one spelling for exact, and
    `==` fails with a hint to use it.
  - `name@version` stays a CLI shorthand (`lvpm install`), as in
    `cargo add`. Once the grammar lands, `install <name>` saves `=x` when
    given a version and a caret on the resolved version otherwise, as
    `cargo add` does; today it saves the version as given, else exact.
- **Dependency groups, remaining** (groups and `--with` / `--without` /
  `--only` are done, see [named dependency groups](#named-dependency-groups)):
  - **Open decision: default groups.** Only `main` installs by default.
    Whether a `dev` group (or a `[project] default-groups` list) should be
    installed by default, as uv does, is undecided. VIPM installs its dev
    dependencies only with `--dev`, which matches today's behaviour.
  - ~~`lvpm add <pkg> --group <g>`~~: done as `install <pkg> --group <g>`.
  - `lvpm.lock` records every group's packages, each tagged with its groups,
    so changing the selection never re-resolves. `lvpm sync` uninstalls
    packages that only unselected groups need.
- **Per-dependency tables**: ~~`hooks`~~ done (see
  [Dependency tables](#dependency-tables-hooks-and-relink)), as is `relink`;
  `source = "..."` is still open.
- **`[venv]` overrides**: `dir`, `vi-server-port`, `bitness`.
- ~~**Open decision: should lvpm discover `vipm.toml`?**~~ Decided: lvpm
  never reads a `vipm.toml` as a manifest, not even with `--manifest`.
  `lvpm import` writes it into an `lvpm.toml` once (see
  [compatibility.md](compatibility.md)). Still open from that discussion:
  - Should `install --manifest <FILE>` look for the venv from the manifest's
    folder instead of the working directory, so *what* and *where* always
    agree? (See [Finding the manifest](#finding-the-manifest).)
