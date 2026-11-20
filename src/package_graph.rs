//! Attribute scanned VI paths to installed packages and project the link graph.

use crate::install::{self, PackageMetadata};
use crate::sbom_paths::{diagnostic_path, redact_paths};
use crate::{cache, index, spec, target, vi_graph};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub type PackageId = (String, String);

#[derive(Eq, Ord, PartialEq, PartialOrd)]
pub enum ComponentId {
    Package(PackageId),
    Packed(String),
}

pub struct Package {
    pub name: String,
    pub metadata: Option<PackageMetadata>,
    pub matched_files: usize,
}

pub struct PackageGraph {
    pub packages: BTreeMap<PackageId, Package>,
    pub roots: BTreeSet<ComponentId>,
    pub edges: BTreeMap<ComponentId, BTreeSet<ComponentId>>,
    pub issues: BTreeSet<String>,
    pub unowned_external: usize,
}

pub fn project(
    graph: &vi_graph::Graph,
    vipm_database: Option<&Path>,
    output: Option<&crate::sbom_paths::OutputGuard>,
) -> Result<PackageGraph> {
    let mut inventory = Inventory::default();
    let mut paths = BTreeMap::<_, BTreeSet<&str>>::new();
    for (label, file) in &graph.files {
        // A compiled PPL has its own dependency identity in the projected graph.
        if graph.packed.contains_key(label) {
            continue;
        }
        paths.entry(path_key(file)).or_default().insert(label);
        // The install record owns the LLB file, while the linker can name a member.
        for ancestor in file.ancestors().skip(1) {
            if ancestor
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("llb"))
            {
                paths.entry(path_key(ancestor)).or_default().insert(label);
                break;
            }
        }
    }

    let global_store = graph.labview.as_ref().map(target::Roots::labview);
    let venv_store = graph.venv.as_ref().map(crate::venv::Venv::store);
    // Keep attribution for links that actually resolve into the installation.
    // Ownership is matched on the physical path, never on a logical VI name.
    for (label, store) in [
        ("venv", venv_store.as_ref()),
        ("labview", global_store.as_ref()),
    ] {
        let Some(store) = store else { continue };
        let (records, unreadable) = install::list_installed_lenient_with(store, |path| {
            output.map_or(Ok(()), |guard| guard.protect(path))
        })?;
        for (path, error) in unreadable {
            let name = path
                .file_name()
                .map_or_else(|| "record".into(), |name| name.to_string_lossy());
            let reason = redact_paths(&format!("{error:#}"));
            inventory.issues.insert(format!(
                "unreadable lvpm ownership record {label}/installed/{name}: {reason}"
            ));
        }
        for record in records {
            let files = matched(&paths, record.files.iter().map(String::as_str));
            inventory.add(record.name, record.version, record.package_metadata, files)?;
        }
    }

    let default_database = graph.labview.as_ref().and_then(default_vipm_database);
    if let Some(database) = vipm_database.or(default_database.as_deref()) {
        inventory.read_vipm(database, &paths, output)?;
    }
    Ok(inventory.collapse(graph))
}

fn default_vipm_database(labview: &target::LvTarget) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let program_data = PathBuf::from(std::env::var_os("ProgramData")?);
    let path = program_data.join("JKI/VIPM/databases").join(format!(
        "LV {:.1} ({}-bit)",
        labview.version, labview.bitness
    ));
    path.is_dir().then_some(path)
}

fn path_key(path: &Path) -> String {
    let text = path.to_string_lossy();
    if cfg!(windows) {
        let normalized = text.replace('\\', "/");
        let normalized = normalized.strip_prefix("//?/").unwrap_or(&normalized);
        normalized.to_lowercase()
    } else {
        text.into_owned()
    }
}

fn matched<'a, 'b>(
    paths: &BTreeMap<String, BTreeSet<&'a str>>,
    records: impl Iterator<Item = &'b str>,
) -> BTreeSet<&'a str> {
    records
        .flat_map(|line| {
            let path = Path::new(line.trim_start_matches('\u{feff}'));
            paths.get(&path_key(path)).into_iter().flatten().copied()
        })
        .collect()
}

#[derive(Default)]
struct Inventory<'a> {
    claims: BTreeMap<&'a str, BTreeSet<PackageId>>,
    packages: BTreeMap<PackageId, Package>,
    issues: BTreeSet<String>,
}

impl<'a> Inventory<'a> {
    fn add(
        &mut self,
        name: String,
        version: String,
        metadata: Option<PackageMetadata>,
        files: BTreeSet<&'a str>,
    ) -> Result<()> {
        if files.is_empty() {
            return Ok(());
        }
        let id = (name.to_ascii_lowercase(), version);
        let package = self.packages.entry(id.clone()).or_insert_with(|| Package {
            name,
            metadata: None,
            matched_files: 0,
        });
        if let (Some(existing), Some(incoming)) = (&package.metadata, &metadata) {
            ensure!(
                existing.archive_sha256 == incoming.archive_sha256,
                "different archives claim package {}@{}",
                id.0,
                id.1
            );
        }
        if package.metadata.is_none() {
            package.metadata = metadata;
        }
        for file in files {
            self.claims.entry(file).or_default().insert(id.clone());
        }
        Ok(())
    }

    fn read_vipm(
        &mut self,
        database: &Path,
        paths: &BTreeMap<String, BTreeSet<&'a str>>,
        output: Option<&crate::sbom_paths::OutputGuard>,
    ) -> Result<()> {
        let entries = std::fs::read_dir(database).with_context(|| {
            format!(
                "reading VIPM ownership database {}",
                diagnostic_path(database)
            )
        })?;
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let dir = entry.path();
            let inventory = dir.join("files-installed");
            if let Some(output) = output {
                output.protect(&inventory)?;
            }
            let installed_files = match std::fs::read(inventory) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    self.issues.insert(format!(
                        "reading VIPM files-installed in {}: {error}",
                        entry.file_name().to_string_lossy()
                    ));
                    continue;
                }
            };
            // The 2026 database stores one absolute path per line. Decode
            // per line so an unrelated legacy path cannot hide valid claims.
            let files = matched(
                paths,
                installed_files
                    .split(|byte| *byte == b'\n')
                    .filter_map(|line| {
                        std::str::from_utf8(line.strip_suffix(b"\r").unwrap_or(line)).ok()
                    }),
            );
            if files.is_empty() {
                continue;
            }
            let archive = match vipm_archive_path(&dir) {
                Ok(archive) => archive,
                Err(error) => {
                    self.issues.insert(format!(
                        "VIPM record {}: {error:#}",
                        entry.file_name().to_string_lossy()
                    ));
                    continue;
                }
            };
            if let Some(output) = output {
                output.protect(&archive)?;
            }
            match read_vipm_archive(&dir, &archive) {
                Ok((name, version, metadata)) => self.add(name, version, Some(metadata), files)?,
                Err(error) => {
                    self.issues.insert(format!(
                        "VIPM record {}: {error:#}",
                        entry.file_name().to_string_lossy()
                    ));
                }
            }
        }
        Ok(())
    }

    fn collapse(mut self, graph: &vi_graph::Graph) -> PackageGraph {
        let mut owners = BTreeMap::new();
        let mut package_files = BTreeMap::<PackageId, Vec<&str>>::new();
        for (file, claims) in self.claims {
            if claims.len() != 1 {
                self.issues.insert(format!(
                    "{file} belongs to multiple packages: {}",
                    claims
                        .iter()
                        .map(|(name, version)| format!("{name}@{version}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                continue;
            }
            let owner = claims.into_iter().next().expect("one owner");
            package_files.entry(owner.clone()).or_default().push(file);
            owners.insert(file, owner);
        }
        self.packages.retain(|id, package| {
            package.matched_files = package_files.get(id).map_or(0, Vec::len);
            package.matched_files > 0
        });
        let unowned_external = graph
            .files
            .keys()
            .filter(|label| {
                !label.starts_with("project/")
                    && !owners.contains_key(label.as_str())
                    && !graph.packed.contains_key(*label)
            })
            .count();
        let roots = first_components(
            graph,
            &owners,
            graph.roots.iter().map(String::as_str).collect(),
            None,
        );
        let mut edges: BTreeMap<_, _> = package_files
            .into_iter()
            .map(|(id, files)| {
                let direct = first_components(graph, &owners, files, Some(&id));
                (ComponentId::Package(id), direct)
            })
            .collect();
        for label in graph.packed.keys() {
            if let Some(direct) = graph.edges.get(label) {
                edges.insert(
                    ComponentId::Packed(label.clone()),
                    direct.iter().cloned().map(ComponentId::Packed).collect(),
                );
            }
        }
        PackageGraph {
            packages: self.packages,
            roots,
            edges,
            issues: self.issues,
            unowned_external,
        }
    }
}

/// Walk through unowned files and files owned by the current package.
/// Stop each path at a different package or a compiled PPL.
fn first_components<'a>(
    graph: &'a vi_graph::Graph,
    owners: &BTreeMap<&str, PackageId>,
    mut pending: Vec<&'a str>,
    from: Option<&PackageId>,
) -> BTreeSet<ComponentId> {
    let mut visited = BTreeSet::new();
    let mut found = BTreeSet::new();
    while let Some(file) = pending.pop() {
        if !visited.insert(file) {
            continue;
        }
        if graph.packed.contains_key(file) {
            found.insert(ComponentId::Packed(file.to_owned()));
            continue;
        }
        if let Some(owner) = owners.get(file)
            && Some(owner) != from
        {
            found.insert(ComponentId::Package(owner.clone()));
            continue;
        }
        pending.extend(graph.edges[file].iter().map(String::as_str));
    }
    found
}

fn vipm_archive_path(dir: &Path) -> Result<PathBuf> {
    let mut archives = Vec::new();
    for file in std::fs::read_dir(dir)? {
        let file = file?;
        let path = file.path();
        if file.file_type()?.is_file()
            && path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("vip") || ext.eq_ignore_ascii_case("ogp")
            })
        {
            archives.push(path);
        }
    }
    ensure!(
        archives.len() == 1,
        "expected one installed package archive, found {}",
        archives.len()
    );
    Ok(archives.pop().expect("one archive"))
}

fn read_vipm_archive(dir: &Path, archive: &Path) -> Result<(String, String, PackageMetadata)> {
    let bytes = std::fs::read(archive)?;
    let md5 = index::md5_hex(&bytes);
    let sha256 = cache::sha256_hex(&bytes);
    let mut zip = install::open_archive(bytes)?;
    let spec = spec::parse(&install::read_spec(&mut zip)?)?;
    ensure!(
        !spec.version.is_empty()
            && dir
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case(&spec.name)),
        "package name or version does not match the installation record"
    );
    let metadata = PackageMetadata {
        vendor: spec.vendor,
        license: spec.license,
        archive_md5: md5,
        archive_sha256: sha256,
    };
    Ok((spec.name, spec.version, metadata))
}
