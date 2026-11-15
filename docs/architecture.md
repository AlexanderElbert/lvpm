# Architecture

## Lineage

The package format lvpm reads comes from the **OpenG Package Tools**
(2002–2005). They defined the package (a zip with a `spec` file), the INI
package directory, `Target Dir` tokens, file groups, replace modes,
install-time script VIs, the installed-package database, dependency
resolution, and a per-project "development system configuration" that could
be exported and imported again. VIPM builds on the same design. The OpenG
documents and source are still online at <https://ogpm.sourceforge.net/>.

lvpm uses the same formats, so today's `.vip` and `.ogp` packages and the
public `.ogpd` / `.vipr` directories work with it unchanged. Link repair is
done in Rust on the files themselves. Work that needs LabVIEW, running hook
VIs and the optional load-and-save relink, goes over VI Server; its TCP
protocol is described in [vi-server-protocol.md](vi-server-protocol.md).
[ogpm-model.md](ogpm-model.md) maps each OpenG term to its lvpm equivalent.

## Source layout

| Module                         | Responsibility                                                                                                          |
| ------------------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| `main.rs`                      | The CLI: commands, target and venv selection, and install/uninstall orchestration                                       |
| `logging.rs`                   | The log files (one per venv, in a global folder), and the `log!` family of macros that print a line and log it                                    |
| `index.rs`                     | Fetch and parse package directories (`.ogpd` / `index.vipr`), resolve names to versions                                |
| `resolve.rs`                   | Resolve requested packages and dependencies into an installation order                                                  |
| `dependency.rs`                | Package dependency metadata and dependency-aware uninstall planning                                                     |
| `fetch.rs`                     | Acquire package bytes: cache lookup, downloads, integrity verification, and cache population                            |
| `cache.rs`                     | The cache: the SQLite index of known sources and the content-addressed store of what they served                        |
| `spec.rs`                      | The `spec` manifest inside a package (the OGPT `.ogp` format and VIPM's `.vip` dialect)                                 |
| `project.rs`                   | The project manifest `lvpm.toml`: dependencies and their constraints, sources, minimum LabVIEW, lookup from a directory |
| `target.rs`                    | LabVIEW detection, and where each `Target Dir` token points                                                             |
| `install.rs`                   | Plan, unpack, extract hook VIs, record a manifest, uninstall                                                            |
| `venv.rs`                      | A project's `.lv-venv/` venv by default: binding, lookup from the working directory, `lvpm venv`                       |
| `launch.rs`                    | Start LabVIEW on a venv: the `-pref` ini, readiness by handshake, detached spawning                                     |
| `linkfix.rs`                   | Rewrite installed VIs' linker tables and separate compiled code, without LabVIEW (see [linkfix.md](linkfix.md))         |
| `relink.rs`                    | Drive the shipped relink VI over installed folders; folder collapsing and retries                                       |
| `refresh.rs`                   | Rebuild palettes and menus through LabVIEW's own shipping VIs                                                           |
| `viserver.rs`                  | The VI Server TCP protocol: connection, methods, flattened data (see [vi-server-protocol.md](vi-server-protocol.md))   |
| `version.rs`                   | Package version ordering: OGPT `version-release`, VIPM's four-part form; not semver                                     |
| `vipm_interop.rs`              | Reading VIPM's `vipm.toml` as an lvpm manifest                                                                          |
| `src/lv-src/relink-package.vi` | The relink VI, shipped next to the executable; `build.rs` records its hash (LabVIEW 2020)                               |
| `scripts/install.ps1`          | Download, verify and install the latest release (the MSI, or the zip with `-Portable`)                                 |
| `scripts/install.sh`           | The same for Linux x64, from the static tarball                                                                         |
| `dist-workspace.toml`, `wix/`  | Releases: dist builds the archives and the per-user MSI when a version tag is pushed (`release.yml`)                    |
| `e2e/`                         | `lvpm-e2e`: end-to-end checks against LabVIEW for Linux and for Windows in NI's container images, with local test packages and with public ones (the LabVIEW tree must come out of install and uninstall unchanged) |
| `tools/linkfix/`               | Python tools for inspecting what linkfix and LabVIEW saves change in a package                                          |
