# Configuration

## Settings

Settings are named the way npm names `npm_config_<key>`: the environment
variable is `LVPM_CONFIG_<KEY>` (or lowercase `lvpm_config_<key>`), and an
empty value counts as unset. Precedence, highest first:

1. Environment variable `LVPM_CONFIG_<KEY>`.
2. User config file at `%LOCALAPPDATA%\lvpm\config.toml` (Linux:
  `$XDG_CONFIG_HOME/lvpm/config.toml`, else `~/.config/lvpm/config.toml`),
  written by `lvpm config set`.
3. Built-in default.

| Variable | Default | What it sets |
| --- | --- | --- |
| `LVPM_CONFIG_CACHE` | `%LOCALAPPDATA%\lvpm\cache` (Linux: `$XDG_CACHE_HOME/lvpm/cache`, else `~/.cache/lvpm/cache`) | Where the cache DB, feed bodies, and imported package archives live |
| `LVPM_CONFIG_SOURCES_LOCAL` | Empty | Local `.vip` directories; separate paths with `;` on Windows or `:` on Linux |
| `LVPM_CONFIG_VENV_DIR` | `.lv-venv` | Project-relative venv directory (or an absolute path) |
| `LVPM_CONFIG_LOG_DIR` | `%LOCALAPPDATA%\lvpm\logs` (Linux: `lvpm/logs` in the cache folder) | Where lvpm's logs go: `lvpm.log` and one file per venv |

```powershell
lvpm config set sources.local D:\LabVIEW\Packages
lvpm config get sources.local
lvpm config set sources.local                 # clear configured local sources
lvpm config set venv.dir .venv
lvpm config get venv.dir
lvpm config set log.dir D:\lvpm-logs
# Or set LVPM_CONFIG_SOURCES_LOCAL for this shell; it overrides config.toml.
$env:LVPM_CONFIG_SOURCES_LOCAL = 'D:\LabVIEW\Packages;E:\MorePackages'
# Relative venv.dir values resolve from each project root; absolute paths are used as-is.
$env:LVPM_CONFIG_VENV_DIR = '.venv'
lvpm search caraya
```

In CI, set `LVPM_CONFIG_CACHE` on the job to keep the cache on a persistent
volume between runs.

## Logs

lvpm keeps its logs in one folder, `%LOCALAPPDATA%\lvpm\logs` on Windows (the
`log.dir` [setting](#settings) moves it). Each venv has its own continuous log
there, named after its project's path, cut from the left to 80 characters, plus
a hash of the venv's path that keeps similar paths apart:

```text
%LOCALAPPDATA%\lvpm\logs\
  lvpm.log                       commands outside a venv, global installs
  c--git-my-project-3fa2c1d0.log  every command run on the venv of C:\Git\my-project
```

`lvpm venv create` and `lvpm venv status` show the venv's log file. Each command
appends a header (the command, its directory and the venv), everything it
printed, detail it does not print, and how it ended (`---- done` or
`---- failed: …`). Each line carries a UTC timestamp, the process id and a
level:

| Level   | What goes there                                                                        |
| ------- | -------------------------------------------------------------------------------------- |
| `error` | why a command failed                                                                   |
| `warn`  | failed hooks, failed uninstalls, files linkfix skipped, download retries               |
| `info`  | everything the command printed                                                         |
| `debug` | file only: every file linkfix changed, every download (URL, size, time) or cache hit   |
| `trace` | not used yet                                                                           |

`--log-level <LEVEL>` (or `LVPM_LOG`) sets the lowest level written; the default is
`debug`, and `off` writes nothing. The console output is the same at every level.
At 10 MB a log moves to `<name>.log.1`, replacing the previous one. A venv
moved to another path starts a new log; the old one stays. Venvs created
before lvpm 0.3.1 kept their log in `.lv-venv/logs/`, which lvpm no longer
writes to.
