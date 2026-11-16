# Known limitations

- `Replace Mode = If Newer` leaves a file that is already on disk alone
  instead of replacing it.
- Installing a package that is already at that version does nothing, so a
  package VI edited or resaved by mistake cannot be repaired that way;
  `lvpm uninstall` and then `lvpm install` does it.
- No dependency conflict resolution: each package gets the newest version
  meeting the highest floor anything asks for, and a manifest pin always
  wins (with a warning when a dependent asks for more).
- A hook VI's own `error out` is not read back. A hook that fails internally
  but runs to completion reports as ok.
- An install that fails part-way through writing a package's files leaves
  them unrecorded (and, on an upgrade, the old version already removed) until
  the install is run again.
- Windows is the tested platform. Linux is verified against LabVIEW 2026 Q3
  in NI's container image only, and does not yet honour a package's
  `Exclusive_OS` ([linux.md](linux.md)).
- In a venv, `PreInstall` hooks cannot run (the venv's LabVIEW only starts
  once the files are down), and file groups aimed at machine locations
  (`<OS ...>`, `<temp>`) are skipped — both are recorded in the manifest and
  shown by `lvpm list`. A hook that drops palette files into the overlay is
  picked up at the next `lvpm launch`, not by a refresh. Uninstall hooks never
  run in a venv, whose LabVIEW has to be closed before its files can be
  removed.
- Some packages don't work in a venv yet (Panther Dashboard for DQMH), and
  some need their PostInstall hook run by hand (AsciiDoc for LabVIEW).
  [package-notes.md](package-notes.md) lists them.
- `lvpm launch` checks install records, not file contents: a package VI
  edited or resaved since install is not detected.

[roadmap.md](roadmap.md) has what is planned.
