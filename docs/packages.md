# Package compatibility in a venv

Packages that need more than a plain `lvpm install` to work in a venv, and
packages that do not work there yet. Each entry names the version it was
checked with; other versions may behave differently.

Checked against LabVIEW 2026 Q3 (64-bit) on Windows, 2026-11-14, by reading
each hook VI's link table and diagram strings (`tools/linkfix/linkobjs.py`,
`lvkit describe`) and running the hooks that matter with `lvpm run-hooks`.

## Packages that do not work

| Package | Version | Symptom | Cause |
| ------- | ------- | ------- | ----- |
| `pantherlab_lib_panther_dashboard` (Panther Dashboard for DQMH) | 9.4.9.78 | **Tools » Panther Dashboard for DQMH** shows the splash and project-selection screens, the scan runs, and the dashboard window never opens. Its PostInstall hook ran without errors. | Not known yet. Every dependency resolves (`deptree.py check`) and LabVIEW does not crash. `Main.vi` shows its panel only after `Scan DQMH Modules from Project.vi` returns without error, so either the scan returns an error or `Main.vi` is broken in the venv. See below. |

**Panther Dashboard: also affected.** Whatever the cause of the missing
window, the dashboard's DQMH scripting buttons won't find DQMH in a venv.
`Get DQMH Scripting UI.vi` and `Check if HSE Scripting Tools are installed.vi`
build their paths from `App.Dir` + `project/Delacor/DQMH/...`. That is the
LabVIEW installation folder. In a venv, DQMH's `project/` files are under
`.lv-venv/packages/delacor_lib_dqmh_toolkit/`, so both VIs report the tools as
missing. The `PantherDashboard Scripting Hooks` VIs read `App.Dir` the same
way.

## Packages whose PostInstall hook must run

A venv runs no install hooks by default (see
[Which hooks run](../README.md#script-vi-hooks)). The hooks below do work the
package needs. Opt in per package in `lvpm.toml`, or run the hook once by hand:

```toml
[dependencies]
wovalab_lib_asciidoc_for_labview = { version = "1.12.1.492", hooks = "run" }
```

```
lvpm run-hooks wovalab_lib_asciidoc_for_labview
```

| Package | Version | What the hook does | Without it |
| ------- | ------- | ------------------ | ---------- |
| `wovalab_lib_asciidoc_for_labview` | 1.12.1.492 | Checks for the Asciidoctor toolchain (Ruby) and installs or updates it. Windows only. Takes about 40 s. | The toolkit, and Antidoc on top of it, can't render HTML or PDF. |
| `pantherlab_lib_panther_dashboard` | 9.4.9.78 | Moves the DQMH scripting plugins into the default data folder, writes Panther's boolean LabVIEW ini setting, and opens the Panther site. | DQMH's scripting tools don't see Panther's plugins. |

The Asciidoctor toolchain does not go into the venv. The hook installs it
per user under `%LOCALAPPDATA%\Asciidoc For LabVIEW\RubyInstance`, so one
run serves every venv and LabVIEW installation of that user, and removing the
venv leaves it behind.

### Optional

| Package | Version | What the hook does |
| ------- | ------- | ------------------ |
| `delacor_lib_qmh` | 7.1.2.1547 | Adds DQMH to the Functions palette (`Read Palette.vi` / `Write Palette.vi`), and deletes the DQMH install marker. Run it only if you want the palette entry. |

### Not needed

These hooks are safe to leave skipped:

| Package | Version | What the hook does |
| ------- | ------- | ------------------ |
| `delacor_lib_dqmh_toolkit` | 7.1.2.1547 | Deletes the DQMH install marker and the temporary installer slide-show VI, and opens DQMH's post-install UI. |
| `delacor_lib_dqmh_documentation` | 7.1.2.1547 | Deletes the DQMH install marker and refreshes menus. |
| `delacor_lib_delacor_qmh_event_scripter` | 7.1.2.1547 | Deletes the DQMH install marker. |
| `wovalab_lib_antidoc` | 3.3.2.887 | Shows a post-installation dialog and opens the Antidoc documentation. |

**The DQMH install marker.** Each Delacor package's PreInstall hook creates
`DQMH Install In Progress.txt`, and its PostInstall hook deletes it. A venv
runs no PreInstall hooks, so the marker is never created and deleting it does
nothing.

## Packages that need `relink = true`

| Package | Version | Why |
| ------- | ------- | --- |
| `sklein_lib_webview2` | 2.1.1.2 | .NET invoke nodes built against another build of the assembly. Only a load and save in LabVIEW fixes them (see [Dependency tables](manifest.md#dependency-tables-hooks-and-relink)). |

## Hooks linkfix replaces

| Package | Hook | Replaced by |
| ------- | ---- | ----------- |
| `ni_lib_advanced_http_client_api` | Restores the Call Library wildcards VI Package Builder removed. | linkfix's [`clfn-wildcard`](linkfix.md#call-library-wildcards) |
| `dataflow_g_lib_g_image` | Unzips clean copies of its VIs over the installed ones. | linkfix |

## Adding an entry

Check a package's hooks before opting it in:

- `python tools/linkfix/deps.py <pkg>` lists which packages in a closure
  carry install hooks.
- `python tools/linkfix/linkobjs.py show <venv>/.lv-venv/hooks/<pkg>-PostInstall.vi`
  shows what the hook calls.
- `lvkit describe <vi>` shows the hook's diagram: its constants, cases and
  calls.

Record the version you checked.
