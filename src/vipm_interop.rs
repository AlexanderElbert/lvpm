//! VIPM interoperability: VIPM's project files, read for `lvpm import`.
//!
//! lvpm's manifest is `lvpm.toml` only. VIPM's files come in once, through
//! `lvpm import`, which writes them into an `lvpm.toml`:
//!
//! - `vipm.toml` (VIPM 2026 and later) is converted field by field into a
//!   [`Project`] by [`read_toml`]. The mapping follows VIPM's published
//!   documentation (docs.vipm.io) and is listed in docs/compatibility.md;
//!   fields with no lvpm equivalent are named rather than dropped silently.
//! - A `.vipc` (VI Package Configuration) is read by [`read_vipc`].

use crate::project::{Constraint, Group, Project};
use crate::version::Version;
use anyhow::{Context, Result, bail, ensure};
use std::path::Path;

/// VIPM's manifest file name.
pub const TOML_FILE_NAME: &str = "vipm.toml";

/// The lvpm dependency group VIPM's `[dev-dependencies]` becomes.
pub const DEV_GROUP: &str = "dev";

/// A converted `vipm.toml`.
#[derive(Debug)]
pub struct Converted {
    pub project: Project,
    /// `[nipm.dependencies]` as written, for copying into an `lvpm.toml`.
    pub nipm: Vec<(String, toml::Value)>,
    /// Dotted names of the fields lvpm did not use, in file order.
    pub unused: Vec<String>,
}

/// Whether `path` names a `vipm.toml` (case-insensitive, as on Windows).
pub fn is_toml(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case(TOML_FILE_NAME))
}

/// Read and convert a `vipm.toml`.
pub fn read_toml(path: &Path) -> Result<Converted> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    parse_toml(&text).with_context(|| format!("reading VIPM manifest {}", path.display()))
}

pub fn parse_toml(text: &str) -> Result<Converted> {
    let doc: toml::Table = toml::from_str(text)?;
    let mut project = Project::default();
    let mut nipm = Vec::new();
    let mut unused = Vec::new();
    for (key, value) in doc {
        match key.as_str() {
            "project" => {
                for (k, v) in table(&key, value)? {
                    match k.as_str() {
                        "name" => project.name = Some(string(&key, &k, v)?),
                        "version" => project.version = Some(string(&key, &k, v)?),
                        // VIPM pins the year; lvpm reads it as the minimum.
                        "labview-version" => project.labview = Some(string(&key, &k, v)?),
                        _ => unused.push(format!("{key}.{k}")),
                    }
                }
            }
            "dependencies" => {
                for (name, v) in table(&key, value)? {
                    let s = string(&key, &name, v)?;
                    project.dependencies.push((name, exact(&key, &s)?));
                }
            }
            "dev-dependencies" => {
                let mut dev = Group::default();
                for (name, v) in table(&key, value)? {
                    let s = string(&key, &name, v)?;
                    dev.dependencies.push((name, exact(&key, &s)?));
                }
                project.groups.push((DEV_GROUP.to_string(), dev));
            }
            "nipm" => {
                for (k, v) in table(&key, value)? {
                    match k.as_str() {
                        "dependencies" => {
                            nipm = table(&k, v)?.into_iter().collect();
                            project.nipm = nipm.iter().map(|(n, _): &(String, toml::Value)| n.clone()).collect();
                        }
                        _ => unused.push(format!("{key}.{k}")),
                    }
                }
            }
            "build" => unused.extend(table(&key, value)?.keys().map(|k| format!("{key}.{k}"))),
            _ => unused.push(key),
        }
    }
    Ok(Converted { project, nipm, unused })
}

fn table(key: &str, value: toml::Value) -> Result<toml::Table> {
    match value {
        toml::Value::Table(t) => Ok(t),
        other => bail!("{key}: expected a table, got {}", other.type_str()),
    }
}

fn string(table: &str, key: &str, value: toml::Value) -> Result<String> {
    match value {
        toml::Value::String(s) => Ok(s),
        other => bail!("[{table}] {key}: expected a string, got {}", other.type_str()),
    }
}

/// VIPM writes exact versions only and rejects empty ones.
fn exact(table: &str, version: &str) -> Result<Constraint> {
    let v = version.trim();
    if v.is_empty() || v.starts_with(['<', '>', '=', '~', '^', '*']) {
        bail!("[{table}] {version:?}: vipm.toml requires an exact version, e.g. \"1.2.3.4\"");
    }
    Ok(Constraint::Exact(Version::parse(v)))
}

// ---------------------------------------------------------------- .vipc ----
//
// A VI Package Configuration (`.vipc`) is VIPM's older way to share a
// project's packages: a zip holding `config.xml` (the LabVIEW target and the
// packages as `name-version`), every listed package's `.spec` and icon, and,
// for packages marked `Included`, the `.vip` / `.ogp` itself. Checked against
// 25 distinct files written by VIPM 2013 to 2026: all are config version 0.6
// with one target, and `Included` always matches the archive being present.

/// One package listed in a `.vipc`.
#[derive(Debug, Clone, PartialEq)]
pub struct VipcPackage {
    pub name: String,
    /// The version as VIPM wrote it, e.g. `7.1.2.1547` or `1.1-1`.
    pub version: String,
    /// The package names its bundled spec requires, as written there. Empty
    /// when the spec is missing or says nothing.
    pub requires: Vec<String>,
    /// The package archive carried in the `.vipc` (`Included`): its member
    /// name and bytes.
    pub embedded: Option<(String, Vec<u8>)>,
}

/// A `.vipc`, read.
#[derive(Debug)]
pub struct Vipc {
    /// The target as written, e.g. `26.1 (64-bit)`.
    pub target: Option<String>,
    /// The target as an lvpm `labview` year (`26.1` is 2026), when it is one.
    pub labview: Option<String>,
    /// In `config.xml` order.
    pub packages: Vec<VipcPackage>,
    /// Bundled specs that could not be read; their packages count as having
    /// no dependencies.
    pub unreadable_specs: Vec<String>,
    /// `config.xml` entries whose name carries no version (`foo`,
    /// `foo-beta`): left out, so one of them does not cost the rest.
    pub unversioned: Vec<String>,
}

/// A package carried in a `.vipc` that is not the one `config.xml` lists.
#[derive(Debug, PartialEq)]
pub struct Mismatch {
    /// The archive's member name in the `.vipc`.
    pub member: String,
    /// Name and version as `config.xml` lists them.
    pub listed: (String, String),
    /// Name and version the archive's own spec gives.
    pub actual: (String, String),
}

impl Vipc {
    /// Name each carried package the way its own archive does, and return
    /// those `config.xml` listed differently. The archive is what goes into
    /// the cache and what an install unpacks, under the name and version in
    /// its spec; a manifest pinning `config.xml`'s reading of it would ask
    /// for a version that is not there.
    pub fn follow_embedded(&mut self) -> Result<Vec<Mismatch>> {
        let mut mismatches = Vec::new();
        for p in &mut self.packages {
            let Some((member, bytes)) = &p.embedded else { continue };
            let (name, version) = crate::index::package_identity(bytes).with_context(|| format!("reading {member}"))?;
            if !name.eq_ignore_ascii_case(&p.name) || Version::parse(&version) != Version::parse(&p.version) {
                mismatches.push(Mismatch {
                    member: member.clone(),
                    listed: (std::mem::replace(&mut p.name, name.clone()), std::mem::replace(&mut p.version, version.clone())),
                    actual: (name, version),
                });
            }
        }
        Ok(mismatches)
    }

    /// The packages nothing else in the file requires, in file order: what
    /// a project would list by hand. VIPM's own `Dependency` flag does not say
    /// this (it disagrees with the specs in 20 of 25 files checked), so the
    /// specs decide. Packages that only require each other in a cycle would
    /// have no root; the first of each such cycle is kept so none is lost.
    pub fn roots(&self) -> Vec<usize> {
        let required = |i: usize| {
            self.packages.iter().enumerate().any(|(j, p)| {
                j != i && p.requires.iter().any(|r| r.eq_ignore_ascii_case(&self.packages[i].name))
            })
        };
        let mut roots: Vec<usize> = (0..self.packages.len()).filter(|&i| !required(i)).collect();
        loop {
            let reached = self.reachable(&roots);
            match (0..self.packages.len()).find(|i| !reached[*i]) {
                Some(i) => roots.push(i),
                None => break,
            }
        }
        roots.sort();
        roots
    }

    /// For each root (by index), the packages it pulls in, so a package left
    /// out of the manifest can say which root brings it.
    pub fn required_by(&self, roots: &[usize]) -> Vec<Option<usize>> {
        let mut by = vec![None; self.packages.len()];
        for &r in roots {
            let reached = self.reachable(&[r]);
            for (i, hit) in reached.into_iter().enumerate() {
                if hit && i != r && by[i].is_none() && !roots.contains(&i) {
                    by[i] = Some(r);
                }
            }
        }
        by
    }

    fn reachable(&self, from: &[usize]) -> Vec<bool> {
        let mut seen = vec![false; self.packages.len()];
        let mut stack: Vec<usize> = from.to_vec();
        while let Some(i) = stack.pop() {
            if std::mem::replace(&mut seen[i], true) {
                continue;
            }
            for r in &self.packages[i].requires {
                if let Some(j) = self.packages.iter().position(|p| p.name.eq_ignore_ascii_case(r)) {
                    stack.push(j);
                }
            }
        }
        seen
    }
}

/// Whether `path` names a `.vipc` file.
pub fn is_vipc(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vipc"))
}

pub fn read_vipc(path: &Path) -> Result<Vipc> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    parse_vipc(&bytes).with_context(|| format!("reading VI package configuration {}", path.display()))
}

pub fn parse_vipc(bytes: &[u8]) -> Result<Vipc> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("not a zip archive")?;
    let config = member(&mut zip, "config.xml")?.context("no config.xml inside; not a VI package configuration")?;
    let (target, listed) = parse_config(&decode_text(&config))?;

    let mut packages = Vec::new();
    let mut unreadable_specs = Vec::new();
    let mut unversioned = Vec::new();
    for (id, included) in listed {
        let Some((name, version)) = crate::version::split_id(&id).map(|(n, v)| (n, v.raw)) else {
            unversioned.push(id);
            continue;
        };
        let spec_name = format!("{id}.spec");
        let requires = match member(&mut zip, &spec_name)? {
            Some(spec) => match crate::spec::parse(&String::from_utf8_lossy(&spec)) {
                Ok(s) => s
                    .requires
                    .as_deref()
                    .map(crate::index::parse_requires)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|r| r.name)
                    .collect(),
                Err(_) => {
                    unreadable_specs.push(spec_name);
                    Vec::new()
                }
            },
            None => {
                unreadable_specs.push(spec_name);
                Vec::new()
            }
        };
        let embedded = match included {
            false => None,
            true => {
                let mut found = None;
                for ext in ["vip", "ogp"] {
                    let file = format!("{id}.{ext}");
                    if let Some(b) = member(&mut zip, &file)? {
                        found = Some((file, b));
                        break;
                    }
                }
                Some(found.with_context(|| format!("{id} is marked Included but its package is not inside"))?)
            }
        };
        packages.push(VipcPackage { name, version, requires, embedded });
    }
    let labview = target.as_deref().and_then(labview_year);
    Ok(Vipc { target, labview, packages, unreadable_specs, unversioned })
}

/// A zip member by name (case-insensitive, as VIPM writes on Windows).
fn member(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>, name: &str) -> Result<Option<Vec<u8>>> {
    let Some(i) = (0..zip.len())
        .find(|&i| zip.name_for_index(i).is_some_and(|n| n.is_ok_and(|n| n.eq_ignore_ascii_case(name))))
    else {
        return Ok(None);
    };
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut zip.by_index(i)?, &mut buf)?;
    Ok(Some(buf))
}

/// `config.xml` as text. VIPM writes it without an encoding declaration:
/// UTF-8 normally, but older versions wrote the creator's name in the
/// Windows code page (`Creator="Jörg"` as a single byte), so bytes that are
/// not UTF-8 are read as Latin-1 rather than refused.
fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// What `config.xml` says: the target's version text, and every `<Package>`
/// as (`name-version`, included).
type Config = (Option<String>, Vec<(String, bool)>);

fn parse_config(xml: &str) -> Result<Config> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    let mut target = None;
    let mut targets = 0;
    let mut packages = Vec::new();
    let mut current: Option<(Option<String>, bool)> = None;
    loop {
        match reader.read_event().context("config.xml is not well-formed XML")? {
            Event::Start(e) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                match (path.last().map(String::as_str), tag.as_str()) {
                    (Some("VI_Package_Configuration"), "Target") => targets += 1,
                    (Some("Target"), "Package") => current = Some((None, false)),
                    _ => {}
                }
                path.push(tag);
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.decode().context("config.xml text")?),
            Event::GeneralRef(r) => match r.resolve_char_ref()? {
                Some(c) => text.push(c),
                None => text.push_str(match r.decode()?.as_ref() {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    other => bail!("config.xml uses an unknown entity &{other};"),
                }),
            },
            Event::End(_) => {
                let tag = path.pop().unwrap_or_default();
                let parent = path.last().map(String::as_str);
                let value = text.trim().to_string();
                match (parent, tag.as_str(), current.as_mut()) {
                    (Some("Target"), "Version", _) => target = Some(value),
                    (Some("Package"), "Name", Some(p)) => p.0 = Some(value),
                    (Some("Package"), "Included", Some(p)) => p.1 = value.eq_ignore_ascii_case("true"),
                    (Some("Target"), "Package", Some(_)) => {
                        let (name, included) = current.take().expect("matched Some");
                        packages.push((name.context("a <Package> in config.xml has no <Name>")?, included));
                    }
                    _ => {}
                }
                text.clear();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    ensure!(path.is_empty(), "config.xml ends inside <{}>", path.join("><"));
    ensure!(targets <= 1, "config.xml has {targets} targets; lvpm reads configurations with one");
    Ok((target, packages))
}

/// `26.1 (64-bit)` is LabVIEW 2026; `9.0` is 2009. Versions before 2009
/// were not named by year and give none.
fn labview_year(target: &str) -> Option<String> {
    let major: u32 = target.trim().split(['.', ' ']).next()?.parse().ok()?;
    match major {
        9..=99 => Some((2000 + major).to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_the_documented_complete_example() {
        let c = parse_toml(
            r#"
[project]
name = "instrument-control"
version = "1.0.0"
labview-version = "2024"
labview-bitness = 64
description = "Automated instrument control"

[dependencies]
oglib_array = "6.0.1.20"
jki_lib_state_machine = "2.0.0.50"

[dev-dependencies]
caraya = "1.4.5.165"

[nipm.feeds]
ni-daq = "https://download.ni.com/support/nipkg/products/ni-d/ni-daqmx/24.5/released/"

[nipm.dependencies]
ni-daqmx = { version = "24.5.0", feed = "ni-daq" }
ni-visa = "24.0"

[nipm.dev-dependencies]
ni-teststand = "2023.1.0"

[build.main_library]
type = "ppl"
top-level-library = "src/InstrumentControl.lvlib"

[build.package]
type = "vipb"
path = "InstrumentControl.vipb"
"#,
        )
        .unwrap();
        let p = &c.project;
        assert_eq!(p.name.as_deref(), Some("instrument-control"));
        assert_eq!(p.version.as_deref(), Some("1.0.0"));
        assert_eq!(p.labview.as_deref(), Some("2024"));
        assert_eq!(
            p.dependencies,
            vec![
                ("oglib_array".into(), Constraint::Exact(Version::parse("6.0.1.20"))),
                ("jki_lib_state_machine".into(), Constraint::Exact(Version::parse("2.0.0.50"))),
            ]
        );
        assert_eq!(p.nipm, ["ni-daqmx", "ni-visa"]);
        assert_eq!(c.nipm[0].1.get("feed").and_then(|f| f.as_str()), Some("ni-daq"), "kept as written");
        assert_eq!(c.nipm[1].1.as_str(), Some("24.0"));
        assert_eq!(
            p.groups,
            vec![(
                "dev".to_string(),
                Group {
                    include: vec![],
                    dependencies: vec![("caraya".into(), Constraint::Exact(Version::parse("1.4.5.165")))],
                }
            )]
        );
        assert!(p.sources.is_empty());
        assert!(p.default_sources);
        assert_eq!(
            c.unused,
            [
                "project.labview-bitness",
                "project.description",
                "nipm.feeds",
                "nipm.dev-dependencies",
                "build.main_library",
                "build.package",
            ]
        );
    }

    #[test]
    fn dev_dependencies_are_selected_like_any_lvpm_group() {
        let c = parse_toml("[dependencies]\na = \"1.0\"\n[dev-dependencies]\nb = \"2.0\"\n").unwrap();
        let none: Vec<String> = Vec::new();
        let names = |s: crate::project::Selection| -> Vec<String> {
            s.dependencies.into_iter().map(|(n, _)| n).collect()
        };
        assert_eq!(names(c.project.select(&none, &none, &none).unwrap()), ["a"]);
        assert_eq!(names(c.project.select(&["dev".into()], &none, &none).unwrap()), ["a", "b"]);
        assert!(parse_toml("[dev-dependencies]\nb = \">=2.0\"\n").is_err(), "exact only");
    }

    #[test]
    fn only_exact_versions_are_accepted() {
        for bad in ["\"\"", "\"  \"", "\">=1.0\"", "\"*\"", "\"=1.0\"", "\"^1.0\""] {
            let text = format!("[dependencies]\noglib_array = {bad}\n");
            assert!(parse_toml(&text).is_err(), "{bad} should be rejected");
        }
        assert!(parse_toml("[dependencies]\noglib_array = 6.0\n").is_err(), "unquoted");
    }

    #[test]
    fn wrong_shapes_are_reported_by_key() {
        let err = parse_toml("project = \"x\"\n").unwrap_err().to_string();
        assert!(err.contains("project"), "{err}");
        let err = parse_toml("[project]\nlabview-version = 2024\n").unwrap_err().to_string();
        assert!(err.contains("labview-version"), "{err}");
    }

    #[test]
    fn recognises_vipm_toml_by_file_name_only() {
        assert!(is_toml(Path::new("C:/proj/vipm.toml")));
        assert!(is_toml(Path::new("VIPM.TOML")));
        assert!(!is_toml(Path::new("C:/proj/lvpm.toml")));
        assert!(!is_toml(Path::new("C:/vipm.toml/lvpm.toml")));
    }

    /// A `.vipc` as VIPM writes it: `config.xml`, one spec per package and,
    /// for `(id, Some(spec))` in `embedded`, the package itself.
    fn vipc(config: &[u8], specs: &[(&str, &str)], embedded: &[(&str, &str)]) -> Vec<u8> {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("config.xml", opts).unwrap();
        w.write_all(config).unwrap();
        for (id, spec) in specs {
            w.start_file(format!("{id}.spec"), opts).unwrap();
            w.write_all(spec.as_bytes()).unwrap();
            w.start_file(format!("{id}.bmp"), opts).unwrap();
        }
        for (file, spec) in embedded {
            let mut pkg = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            pkg.start_file("spec", opts).unwrap();
            pkg.write_all(spec.as_bytes()).unwrap();
            let bytes = pkg.finish().unwrap().into_inner();
            w.start_file(*file, opts).unwrap();
            w.write_all(&bytes).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn config(target: &str, packages: &[(&str, bool)]) -> String {
        let mut s = String::from(
            "<VI_Package_Configuration File_Type=\"xml\" Version=\"0.6\" Creator=\"x\">\r\n  <Target>\r\n    <Name>LabVIEW</Name>\r\n",
        );
        s.push_str(&format!("    <Version>{target}</Version>\r\n"));
        for (id, included) in packages {
            s.push_str(&format!(
                "    <Package>\r\n      <Name>{id}</Name>\r\n      <Included>{included}</Included>\r\n      \
                 <Dependency>true</Dependency>\r\n      <Pinned>false</Pinned>\r\n    </Package>\r\n"
            ));
        }
        s.push_str("  </Target>\r\n</VI_Package_Configuration>");
        s
    }

    fn spec(name: &str, version: &str, requires: &str) -> String {
        format!("[Package]\nName=\"{name}\"\nVersion=\"{version}\"\n[Dependencies]\nRequires=\"{requires}\"\nConflicts=\"\"\n")
    }

    fn names(v: &Vipc, idx: &[usize]) -> Vec<String> {
        idx.iter().map(|&i| v.packages[i].name.clone()).collect()
    }

    #[test]
    fn a_vipc_reads_its_target_packages_and_roots_from_the_specs() {
        let cfg = config(
            "26.1 (64-bit)",
            &[("ig_http_server-0.5.3.36", false), ("ig_stream_base-1.2.0.46", false), ("ig_web&amp;crypto-0.1.0.9", false), ("delacor_lib_qmh-7.1.2.1547", false)],
        );
        let bytes = vipc(
            cfg.as_bytes(),
            &[
                ("ig_http_server-0.5.3.36", &spec("ig_http_server", "0.5.3.36", "ig_stream_base>=1.2.0.46,ig_web&crypto>=0.1.0.9")),
                ("ig_stream_base-1.2.0.46", &spec("ig_stream_base", "1.2.0.46", "")),
                ("ig_web&crypto-0.1.0.9", &spec("ig_web&crypto", "0.1.0.9", "IG_STREAM_BASE>=1.0")),
                ("delacor_lib_qmh-7.1.2.1547", &spec("delacor_lib_qmh", "7.1.2.1547", "")),
            ],
            &[],
        );
        let v = parse_vipc(&bytes).unwrap();
        assert_eq!(v.target.as_deref(), Some("26.1 (64-bit)"));
        assert_eq!(v.labview.as_deref(), Some("2026"));
        assert_eq!(v.packages[2].name, "ig_web&crypto", "entities are decoded");
        assert_eq!(v.packages[0].version, "0.5.3.36");
        assert!(v.unreadable_specs.is_empty());
        let roots = v.roots();
        assert_eq!(names(&v, &roots), ["ig_http_server", "delacor_lib_qmh"], "file order, names case-insensitive");
        let by = v.required_by(&roots);
        assert_eq!(by, [None, Some(0), Some(0), None]);
    }

    #[test]
    fn included_packages_come_out_of_the_vipc_with_their_bytes() {
        let cfg = config("20.0", &[("jki_rsc_toolkits_palette-1.1-1", true), ("my_lib-1.0.0.1", true), ("oglib_error-4.2.0.23", false)]);
        let bytes = vipc(
            cfg.as_bytes(),
            &[
                ("jki_rsc_toolkits_palette-1.1-1", &spec("jki_rsc_toolkits_palette", "1.1", "")),
                ("my_lib-1.0.0.1", &spec("my_lib", "1.0.0.1", "oglib_error>=4.0,jki_rsc_toolkits_palette")),
                ("oglib_error-4.2.0.23", &spec("oglib_error", "4.2.0.23", "")),
            ],
            &[("jki_rsc_toolkits_palette-1.1-1.ogp", &spec("jki_rsc_toolkits_palette", "1.1", "")), ("my_lib-1.0.0.1.vip", &spec("my_lib", "1.0.0.1", ""))],
        );
        let v = parse_vipc(&bytes).unwrap();
        assert_eq!(v.packages[0].version, "1.1-1", "the OGPM release suffix stays on the version");
        assert_eq!(v.packages[0].embedded.as_ref().unwrap().0, "jki_rsc_toolkits_palette-1.1-1.ogp");
        assert_eq!(v.packages[1].embedded.as_ref().unwrap().0, "my_lib-1.0.0.1.vip");
        assert!(v.packages[2].embedded.is_none());
        assert_eq!(names(&v, &v.roots()), ["my_lib"]);
        assert_eq!(v.labview.as_deref(), Some("2020"));

        let missing = config("20.0", &[("gone-1.0", true)]);
        let err = parse_vipc(&vipc(missing.as_bytes(), &[], &[])).unwrap_err();
        assert!(format!("{err:#}").contains("marked Included"), "{err:#}");
    }

    #[test]
    fn a_cycle_keeps_one_of_its_packages_and_a_missing_spec_is_reported() {
        let cfg = config("24.3 (64-bit)", &[("a-1.0", false), ("b-1.0", false), ("c-1.0", false)]);
        let bytes = vipc(cfg.as_bytes(), &[("a-1.0", &spec("a", "1.0", "b")), ("b-1.0", &spec("b", "1.0", "a"))], &[]);
        let v = parse_vipc(&bytes).unwrap();
        assert_eq!(v.unreadable_specs, ["c-1.0.spec"]);
        assert_eq!(names(&v, &v.roots()), ["a", "c"], "c has no spec, so it is a root; the a/b cycle keeps a");
    }

    #[test]
    fn a_config_in_the_windows_code_page_still_reads() {
        let mut cfg = config("20.0", &[("x-1.0", false)]).into_bytes();
        let at = cfg.windows(9).position(|w| w == b"Creator=\"").unwrap() + 9;
        cfg.splice(at..at + 1, b"J\xf6rg".iter().copied());
        assert!(std::str::from_utf8(&cfg).is_err());
        let v = parse_vipc(&vipc(&cfg, &[("x-1.0", &spec("x", "1.0", ""))], &[])).unwrap();
        assert_eq!(v.packages.len(), 1);
    }

    #[test]
    fn a_carried_package_is_named_by_its_own_spec_when_config_xml_disagrees() {
        let cfg = config("24.3 (64-bit)", &[("same-1.0.0.1", true), ("rebuilt-1.0.0.1", true), ("listed_only-2.0", false)]);
        let bytes = vipc(
            cfg.as_bytes(),
            &[],
            &[("same-1.0.0.1.vip", &spec("Same", "1.0.0.1", "")), ("rebuilt-1.0.0.1.vip", &spec("rebuilt", "1.0.0.2", ""))],
        );
        let mut v = parse_vipc(&bytes).unwrap();
        let mismatches = v.follow_embedded().unwrap();
        assert_eq!(
            mismatches,
            [Mismatch {
                member: "rebuilt-1.0.0.1.vip".into(),
                listed: ("rebuilt".into(), "1.0.0.1".into()),
                actual: ("rebuilt".into(), "1.0.0.2".into()),
            }]
        );
        let pins: Vec<(&str, &str)> = v.packages.iter().map(|p| (p.name.as_str(), p.version.as_str())).collect();
        assert_eq!(pins, [("same", "1.0.0.1"), ("rebuilt", "1.0.0.2"), ("listed_only", "2.0")], "a name differing only in case is the same package");

        // A carried file that is not a package stops the import before anything is written.
        let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opts = zip::write::SimpleFileOptions::default();
        use std::io::Write;
        w.start_file("config.xml", opts).unwrap();
        w.write_all(config("24.3", &[("junk-1.0", true)]).as_bytes()).unwrap();
        w.start_file("junk-1.0.vip", opts).unwrap();
        w.write_all(b"not a zip").unwrap();
        let mut v = parse_vipc(&w.finish().unwrap().into_inner()).unwrap();
        let err = format!("{:#}", v.follow_embedded().unwrap_err());
        assert!(err.contains("junk-1.0.vip"), "{err}");
    }

    #[test]
    fn an_entry_without_a_version_is_set_aside_and_the_rest_still_read() {
        let cfg = config("25.3 (64-bit)", &[("good-1.0.0.1", false), ("foo", false), ("foo-beta", false)]);
        let v = parse_vipc(&vipc(cfg.as_bytes(), &[("good-1.0.0.1", &spec("good", "1.0.0.1", ""))], &[])).unwrap();
        assert_eq!(names(&v, &v.roots()), ["good"]);
        assert_eq!(v.unversioned, ["foo", "foo-beta"]);
        assert!(v.unreadable_specs.is_empty(), "an entry left out is not also reported for its spec");
    }

    #[test]
    fn empty_and_malformed_configurations() {
        let v = parse_vipc(&vipc(config("25.3 (64-bit)", &[]).as_bytes(), &[], &[])).unwrap();
        assert!(v.packages.is_empty() && v.roots().is_empty());
        assert_eq!(v.labview.as_deref(), Some("2025"));
        assert!(parse_vipc(b"not a zip").is_err());
        assert!(parse_vipc(&vipc(b"<VI_Package_Configuration><Target>", &[], &[])).is_err());
        let two = "<VI_Package_Configuration><Target></Target><Target></Target></VI_Package_Configuration>";
        assert!(parse_vipc(&vipc(two.as_bytes(), &[], &[])).is_err());
    }

    #[test]
    fn targets_map_to_years_from_2009() {
        assert_eq!(labview_year("26.1 (64-bit)").as_deref(), Some("2026"));
        assert_eq!(labview_year("13.0").as_deref(), Some("2013"));
        assert_eq!(labview_year("9.0").as_deref(), Some("2009"));
        assert_eq!(labview_year("8.6"), None);
        assert_eq!(labview_year("LabVIEW"), None);
        assert!(is_vipc(Path::new("C:/p/Deps.VIPC")));
        assert!(!is_vipc(Path::new("C:/p/deps.vipc.zip")));
    }

    #[test]
    fn a_minimal_file_converts_to_an_empty_project() {
        let c = parse_toml("[project]\nname = \"x\"\n").unwrap();
        assert!(c.project.dependencies.is_empty());
        assert!(c.unused.is_empty());
    }
}
