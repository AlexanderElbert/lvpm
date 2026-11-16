# G-CLI compatibility — proposal

Status: proposal, nothing implemented.

[G-CLI](https://github.com/G-CLI/G-CLI) is how many LabVIEW tools and CI
scripts run a VI from the command line (`g-cli vipc -- deps.vipc`,
`g-cli my_tool.vi -- --flag`). It cannot run a VI on an lvpm venv: it always
starts the installation's LabVIEW with the installation's ini. This document
proposes that lvpm provide G-CLI itself — the `g-cli` command, `lvpm run`, and
G-CLI's LabVIEW library — so existing scripts keep working unchanged and a
separate G-CLI install is no longer needed.

## How G-CLI works today

Read from G-CLI 3.0.1 (`rust-proxy/src/`, `docs/dev/lv-cli-messaging.md`).
G-CLI has two halves: a Rust command-line proxy, `g-cli`, and a LabVIEW
library that the VI being run calls.

1. **Install detection.** Windows: the registry under
   `HKLM\SOFTWARE\National Instruments\LabVIEW` (and `WOW6432Node`). Linux:
   `/usr/local/natinst/labview-<ver>-64`. `--lv-ver` / `--arch` select one;
   no match falls back to the default install.
2. **VI resolution.** A missing path is retried under
   `<install>/vi.lib/G CLI Tools/` (how named tools like `vipc` work). No
   extension means `.vi`; a `.exe` is launched without LabVIEW. Paths inside
   `.lvlibp` / `.llb` are supported.
3. **Port.** The proxy binds `127.0.0.1:0` and registers the port with the
   NI Service Locator:
   `GET localhost:3580/publish?cli/<major>/<bitness>/<vi path with : \ . space / ? stripped>=…Port=<port>`.
   The LabVIEW library looks this key up from its own VI path, so it does not
   matter who launched the LabVIEW running the VI.
4. **Launch.** `LabVIEW.exe "<vi>" -unattended -- -p:<port>` (`-unattended`
   dropped by `--allow-dialogs`), detached, no inherited handles. A thread
   polls once a second that a process with the LabVIEW exe path is alive.
5. **Connect back.** The VI calls *Start CLI Interface*, which connects to
   the port (60 s default `--connect-timeout`). The proxy then deletes the
   Service Locator entry and sends `ARGS` and `CCWD`.
6. **Protocol.** Frames are `u32 big-endian length | 4-char ID | payload`,
   at most 9000 bytes.

   | Direction | ID     | Payload                              |
   |-----------|--------|--------------------------------------|
   | → LabVIEW | `ARGS` | user arguments, tab separated        |
   | → LabVIEW | `CCWD` | caller's working directory           |
   | → g-cli   | `OUTP` | text for stdout                      |
   | → g-cli   | `SERR` | text for stderr                      |
   | → g-cli   | `OFLS` | flush stdout                         |
   | → g-cli   | `EXIT` | exit code as a decimal string        |

7. **Exit.** `EXIT` ends the proxy with that code. LabVIEW is left running
   unless `--kill` (after `--kill-timeout`). Ctrl-C kills LabVIEW and exits
   with -1; a dropped connection exits with -1.

G-CLI is distributed as the VI package `Wiresmith_Technology_lib_G_CLI`. It
installs the LabVIEW library to `<vi.lib>\G CLI Tools`, and its post-install
VI runs a bundled MSI that installs `g-cli.exe` machine-wide and appends it
to the **system** PATH.

## Why it cannot run on a venv

A venv is activated by starting LabVIEW with `-pref .lv-venv/labview.ini` and
`TEMP`/`TMP` pointed into the venv (`src/launch.rs`). G-CLI's options are
`--lv-ver`, `--arch`/`--x64`, `--connect-timeout`, `--kill`, `--kill-timeout`,
`--allow-dialogs`, `--verbose` and the deprecated `--no-launch`: there is no
way to pass LabVIEW arguments, an ini, or environment variables, and the
install path comes from the registry.

- With no LabVIEW running, G-CLI starts a global LabVIEW: no venv packages,
  the user's cache and `LabVIEW Data`, and a save relinks against global
  packages.
- With LabVIEW already running, the new `LabVIEW.exe` hands the VI to the
  most recently started instance and exits. That can be the venv IDE or the
  primary IDE, depending only on which was opened last (see
  [Handing a VI to a running LabVIEW](#handing-a-vi-to-a-running-labview-measured)).

### Handing a VI to a running LabVIEW (measured)

Measured on LabVIEW 2026 64-bit (v26.3), Windows 11, 2026-11-16. A primary
IDE (global ini) and a venv IDE (`lvpm launch`) were both open, and three
copies of a harmless `vi.lib` VI were opened with `LabVIEW.exe`, each copy
named for its case. The process whose window showed the copy's name had
received it. Every process a case started was closed before the next case.

| Case | Command | Primary started first | Venv started first |
|---|---|---|---|
| A | `LabVIEW.exe <vi> -unattended` (what G-CLI runs) | venv IDE | primary IDE |
| B | `LabVIEW.exe -pref <venv ini> <vi> -unattended` | new process | new process |
| C | as B, with `AllowMultipleInstances=False` in a copy of the venv ini | venv IDE | primary IDE |

- **The handoff goes to the most recently started instance, whatever the
  ini.** In A and C the new process exited within about a second and the VI
  opened in the IDE started last. `-pref` does not choose the target.
- **`AllowMultipleInstances=True` always starts a new process** (B), which
  is why lvpm sets it and why `launch::is_running` must stop a second launch
  on the same venv.
- **There is no way to direct `LabVIEW.exe` at a particular instance.**
  Reaching a specific running LabVIEW takes its VI Server port, which is
  per instance (`port_for`).
- Not separated yet: "most recently started" and "most recently focused".
  The IDE started last was also the last to get focus.

So today's `g-cli` already runs in a venv IDE by accident when that IDE was
opened after the primary one, and in the primary IDE otherwise.

Installing G-CLI into a venv is worse still: lvpm runs post-install hooks, so
the package's MSI would install a machine-wide `g-cli` from a venv install.

## Proposal

### 1. `lvpm run` and `g-cli` are one program

```
lvpm [--project DIR | --global] run [g-cli options] <vi|tool> [-- args]
g-cli [g-cli options] <vi|tool> [-- args]
```

- Everything after `run` is parsed by G-CLI's own argument parser, taken
  unchanged, so `lvpm run --lv-ver 2025 --kill vipc -- -v 2025 deps.vipc`
  means exactly what the same line means to upstream `g-cli`. lvpm's
  options stay before `run`; none of G-CLI's names overlap with
  `--project`, `--global` or `--labview-version` (any lvpm flag that does
  collide, such as a `--verbose`, is only accepted before `run`).
- lvpm ships `g-cli(.exe)` beside `lvpm(.exe)`: the same binary dispatching
  on the name it was started as, or a minimal launcher that execs
  `lvpm run "$@"`. With no separate G-CLI installed, nothing else on PATH is
  called `g-cli`, so lvpm's own install directory on PATH is enough — no
  activation, no wrapper. The venv is found from the working directory, as
  every other lvpm command finds it.

Existing CI scripts keep calling `g-cli …` and, inside a project, get the
venv.

### 2. Vendored, not reimplemented

G-CLI is BSD-2-Clause. lvpm vendors `rust-proxy` (argument parsing in
`cli.rs`, the protocol in `comms.rs`, the action / comms / signal loops,
install detection) and changes only `launch_process` in `main.rs`. Vendoring
gets exact compatibility for free — option aliases (`--timeout` /
`--connect-timeout`, `--allowDialogs`), how arguments are split around `--`,
the `.vi` default, exit codes — that a rewrite would drift from. The Service
Locator key is computed by G-CLI's own code, so it matches what the LabVIEW
library looks up. Copyright and licence text are kept and recorded in
`PROVENANCE.md`.

The replacement `launch_process`:

1. Resolve the venv as lvpm does (`--project`, `LVPM_PROJECT` for the
   `g-cli` entry point since G-CLI's flags cannot carry it, or upwards from
   the working directory). `--global`, or no venv, means upstream behaviour.
2. In a venv: write the venv ini (`launch::write_ini`), register the port,
   and either start the venv LabVIEW with
   `-pref <ini> <vi> -unattended -- -p:<port>` and the venv `TEMP`
   (`launch::spawn`), or — if one is running (`launch::is_running`) — run the
   VI in it over VI Server (`open_vi_reference` + `run_vi_async`).

### 3. Behaviour in a venv

Outside a venv, or with `--global`, behaviour is upstream's. Inside one:

| | Upstream G-CLI | lvpm |
|---|---|---|
| Launch | `LabVIEW.exe <vi> -unattended -- -p:N` | adds `-pref .lv-venv/labview.ini` and the venv `TEMP` |
| LabVIEW already running | launches again; the VI is handed to the most recently started instance, venv or primary (measured above) | runs the VI in the venv instance over VI Server. A second `-pref` launch would start a second instance on the same cache (`AllowMultipleInstances=True`), the SQLITE_BUSY case `is_running` guards against. "Run when opened" does not fire on Open VI Reference, so the VI is run explicitly. |
| `--lv-ver` / `--arch` not the venv's | falls back to the default install | refused: the global install would silently bypass the venv |
| `--kill`, Ctrl-C | kills the LabVIEW process | kills only an instance this run started (`Instance::shutdown_if_spawned`); in a reused instance — possibly the user's `lvpm launch` IDE — the VI is aborted instead |
| Tool lookup | `<install>/vi.lib/G CLI Tools` | each venv package's `1/vi.lib/G CLI Tools` first, then the installation |

### 4. G-CLI is a package lvpm provides

Refusing `Wiresmith_Technology_lib_G_CLI` outright would break every package
that depends on it, and many CLI tools do. Instead:

- lvpm bundles G-CLI's LabVIEW library at the same upstream tag as the
  vendored proxy, and installs it as it installs any `<vi.lib>` content — a
  venv package overlaid onto `vi.lib` via `LVAddons.AdditionalLocations`, or
  the installation's `vi.lib` for `--global`. VIs and dependents link to
  `<vilib>\G CLI Tools\…` exactly as they would to an upstream install.
- The resolver treats `wiresmith_technology_lib_g_cli` as provided by lvpm
  at the bundled version: dependents resolve against it, and it is never
  downloaded.
- `lvpm install wiresmith_technology_lib_g_cli` reports
  `provided by lvpm (G-CLI 3.0.1)` instead of failing; a constraint above the
  bundled version is refused with `needs lvpm ≥ x`.
- The package's post-install MSI never runs, because the package is never
  installed from its `.vip`.

## Upstream: `--lv-ini`

A smaller change in G-CLI itself covers the most common case, a cold start in
CI. Branch [`feature/lv-ini`](https://github.com/labview-package-manager/G-CLI/tree/feature/lv-ini)
on our fork adds `--lv-ini <path>`, also read from `G_CLI_LV_INI`, which puts
`-pref <path>` in front of the VI on G-CLI's LabVIEW command line:

```
LabVIEW.exe -pref <ini> <vi> -unattended -- -p:<port>
```

That is the only thing lvpm cannot pass to G-CLI today:

- The rest of a venv launch already gets through. G-CLI starts LabVIEW with
  `CreateProcessW` and a null environment, so `TEMP`/`TMP` set by the caller
  reach LabVIEW.
- Install detection and the Service Locator key are unchanged. A venv runs
  the installation's own `LabVIEW.exe` (`LvTarget::exe`), so the version and
  bitness that G-CLI registers are still the ones the LabVIEW library looks
  up. An option that replaced the exe path would have broken this.
- The option has uses outside lvpm, such as a build agent's own VI Server
  port or object cache. That makes it a reasonable upstream request.

If upstream merges it, lvpm's `g-cli` / `lvpm run` can wrap the real G-CLI
instead of vendoring its proxy: write the venv ini, set `G_CLI_LV_INI`,
`TEMP` and `TMP` on the child, and exec `g-cli`. These parts stay on lvpm's
side either way:

- reusing a venv LabVIEW that is already running. A second `-pref` launch
  starts a second instance on the same cache (`AllowMultipleInstances=True`),
  and a launch without it is handed to whichever instance started last, not
  necessarily the venv's (measured above). The wrapper has to detect this
  (`launch::is_running`) and either refuse or run the VI over VI Server.
  G-CLI's own startup cannot do it;
- refusing an `--lv-ver`/`--arch` that is not the venv's, which upstream
  quietly falls back from;
- `--kill` only for an instance this run started;
- installing the LabVIEW library and never running the package's MSI.

Wrapping also means a separate G-CLI install to depend on, and its version is
not pinned by lvpm. Choosing between wrapping and vendoring can wait for
upstream's answer. Vendoring stays the fallback.

## Costs

- **G-CLI's version follows lvpm's.** A G-CLI release needs an lvpm release
  that updates both the vendored proxy and the bundled library from the same
  tag. The wire protocol has been stable since 1.5.2 (`EXIT` as a string), so
  a library from another 2.x/3.x release still talks to the proxy.
- **An existing G-CLI install wins on PATH.** The MSI puts `g-cli` on the
  system PATH, which Windows searches before the user PATH. lvpm should
  detect it (`where g-cli`, or the MSI's uninstall entry) and warn that
  `g-cli` calls bypass the venv until it is uninstalled. `lvpm run` is
  unaffected.

## Open questions

- **Service Locator key in a venv.** The LabVIEW library builds the lookup
  key from its own VI path, LabVIEW major version and bitness. For a tool in
  a venv package, check on a real install that the path LabVIEW reports is
  the on-disk `.lv-venv/packages/…` path the proxy registered, not a
  `vi.lib`-overlay path.
- **Start order or focus.** The handoff measurement does not yet separate
  the most recently started instance from the most recently focused one.
  Repeat it with the first IDE brought to the front before the launch.
- **Running a reused instance.** Confirm that a G-CLI VI started with
  `run_vi_async` (front panel not opened) behaves as when opened from the
  command line, including how it closes itself after `EXIT`.
- **Linux.** G-CLI's Linux detection only recognises 64-bit
  `labview-<ver>-64` folders; check that it agrees with lvpm's target
  resolution, or use lvpm's target instead.

## Alternatives considered

- **Replace the installed `g-cli.exe`.** MSI repair and upgrade restore it,
  it modifies a global install that lvpm otherwise never touches, and it
  affects every project.
- **`lvpm run -- <cmd>` putting `.lv-venv/bin` first on PATH.** Only needed
  while a separate G-CLI is installed; with lvpm providing `g-cli` it adds
  nothing.
- **Point G-CLI at a launcher `.exe`.** G-CLI launches an exe with only
  `-- -p:<port>`: no VI path and no Service Locator registration, and its
  process monitoring and `--kill` would act on the launcher, not LabVIEW.
- **Upstream launch hook.** An option such as `G_CLI_LAUNCHER=<cmd>` that
  G-CLI calls instead of starting LabVIEW. G-CLI's process monitoring and
  `--kill` would then track the launcher, not LabVIEW, and the hook would
  need its own contract. Superseded by the smaller `--lv-ini` change in
  [Upstream: `--lv-ini`](#upstream---lv-ini).
