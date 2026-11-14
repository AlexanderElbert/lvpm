# lvpm documentation

## Using lvpm

| Document | What it covers |
| --- | --- |
| [installation.md](installation.md) | Installing lvpm: the install script, by hand, from source, testing a local build |
| [commands.md](commands.md) | Every command and flag; uninstalling with dependencies |
| [manifest.md](manifest.md) | `lvpm.toml`: dependencies, constraints, groups, sources, `hooks` and `relink` |
| [compatibility.md](compatibility.md) | VIPM's files: `lvpm import` of a `vipm.toml` or a `.vipc`, and how VIPM and lvpm differ |
| [venvs.md](venvs.md) | Per-project dependencies: creating a venv, `lvpm launch`, its layout and how the overlay works |
| [install-process.md](install-process.md) | What an install does in LabVIEW: linkfix, relinking, install hooks, VI Server |
| [package-sources.md](package-sources.md) | The public package directories, your own indexes, local folders and the cache |
| [configuration.md](configuration.md) | Settings, environment variables and logs |
| [ci-container.md](ci-container.md) | Running lvpm in NI's LabVIEW containers, headless mode |
| [linux.md](linux.md) | What differs on Linux |
| [package-notes.md](package-notes.md) | Commonly used packages in a venv: whose hooks are needed, which can stay skipped, which don't work yet |
| [limitations.md](limitations.md) | Known limitations |

## Internals and design

| Document | What it covers |
| --- | --- |
| [architecture.md](architecture.md) | Where lvpm comes from (OpenG Package Tools) and the source layout |
| [linkfix.md](linkfix.md) | How linkfix rewrites installed VIs without LabVIEW, and how it is validated |
| [vi-server-protocol.md](vi-server-protocol.md) | The VI Server TCP protocol lvpm speaks |
| [ogpm-model.md](ogpm-model.md) | The OpenG Package Tools model and its map to lvpm |
| [cache-design.md](cache-design.md) | Design of the content-addressed package cache |
| [cli.md](cli.md) | Interface review: what changes before the CLI is called stable |
| [refactor-main.md](refactor-main.md) | Plan for splitting `main.rs` |
| [roadmap.md](roadmap.md) | What is planned |
