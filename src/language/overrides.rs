use super::{LANGUAGES, LanguageConfig};
use crate::index::SymbolKind;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use tree_sitter::{Language, Parser, Query};

pub type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    name: String,
    extensions: Vec<String>,
    grammar: String,
    library: PathBuf,
    query: PathBuf,
    ref_node_types: Vec<String>,
    #[serde(default)]
    sig_body_child: Option<String>,
    #[serde(default)]
    sig_delimiter: Option<u8>,
    #[serde(default)]
    kind_overrides: Vec<(String, String, SymbolKind)>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Registration {
    pub directory: PathBuf,
    pub source: PathBuf,
}

pub struct Override {
    pub config: LanguageConfig,
    pub language: Language,
    pub query: Query,
    // Drop the native library after its language and query.
    _library: libloading::Library,
}

struct State {
    languages: Vec<Override>,
    fingerprint: u64,
}

static STATE: LazyLock<Result<State, Error>> = LazyLock::new(|| {
    let mut languages = Vec::new();
    let mut hash = DefaultHasher::new();
    for (name, registration) in registrations()? {
        name.hash(&mut hash);
        let loaded = load(&registration.directory, &mut hash)
            .map_err(|e| format!("{name}: {e}; replace with `cx lang add {name} --from DIR` or remove with `cx lang remove {name} --override`"))?;
        if loaded.config.name != name {
            return Err(format!("override {name} contains language {}", loaded.config.name).into());
        }
        check_extensions(
            &loaded.config.name,
            &loaded.config.extensions,
            languages
                .iter()
                .map(|o: &Override| (&o.config.name, &o.config.extensions)),
        )?;
        languages.push(loaded);
    }
    Ok(State {
        languages,
        fingerprint: hash.finish(),
    })
});

fn state() -> &'static State {
    STATE.as_ref().unwrap_or_else(|e| {
        eprintln!("cx: invalid language override: {e}");
        std::process::exit(1);
    })
}

pub fn installed() -> &'static [Override] {
    &state().languages
}

pub fn get(name: &str) -> Option<&'static Override> {
    installed().iter().find(|o| o.config.name == name)
}

pub fn fingerprint() -> u64 {
    state().fingerprint
}

fn root() -> PathBuf {
    crate::lang::cx_cache_dir().join("overrides")
}

fn valid_name(name: &str) -> bool {
    name.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

pub fn registrations() -> Result<Vec<(String, Registration)>, Error> {
    if !root().exists() {
        return Ok(Vec::new());
    }
    let _lock = registration_lock(false)?;
    read_registrations()
}

fn registration_lock(exclusive: bool) -> Result<fs::File, Error> {
    fs::create_dir_all(root())?;
    let lock = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root().join(".lock"))?;
    if exclusive {
        lock.lock()?;
    } else {
        lock.lock_shared()?;
    }
    Ok(lock)
}

fn read_registrations() -> Result<Vec<(String, Registration)>, Error> {
    let entries = match fs::read_dir(root()) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut records = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if !valid_name(name) {
            return Err(format!("invalid override name: {}", path.display()).into());
        }
        records.push((name.to_owned(), serde_json::from_slice(&fs::read(&path)?)?));
    }
    records.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(records)
}

fn read_manifest(directory: &Path) -> Result<Manifest, Error> {
    Ok(serde_json::from_slice(&fs::read(
        directory.join("language.json"),
    )?)?)
}

fn check_extensions<'a>(
    name: &str,
    extensions: &[String],
    others: impl Iterator<Item = (&'a String, &'a Vec<String>)>,
) -> Result<(), Error> {
    if extensions.is_empty()
        || extensions.iter().any(|e| {
            e.is_empty()
                || e.contains(['.', '/', '\\'])
                || e.chars().any(|c| c.is_whitespace() || c.is_control())
        })
    {
        return Err(
            "extensions must be nonempty file suffixes without dots, slashes, or whitespace".into(),
        );
    }
    for (other, suffixes) in LANGUAGES
        .iter()
        .map(|c| (&c.name, &c.extensions))
        .chain(others)
    {
        if other != name && extensions.iter().any(|e| suffixes.contains(e)) {
            return Err(format!(
                "extensions for {name} conflict with {other}; override {other} instead"
            )
            .into());
        }
    }
    Ok(())
}

fn library_name(grammar: &str) -> String {
    format!(
        "{}tree_sitter_{grammar}{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    )
}

fn load(directory: &Path, hash: &mut DefaultHasher) -> Result<Override, Error> {
    let manifest_bytes = fs::read(directory.join("language.json"))?;
    manifest_bytes.hash(hash);
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    if !valid_name(&manifest.name) || !valid_name(&manifest.grammar) {
        return Err(
            "language and grammar names must start with a-z and contain only a-z, 0-9, or _".into(),
        );
    }
    if manifest.library != Path::new(&library_name(&manifest.grammar)) {
        return Err("installed parser filename does not match its grammar".into());
    }
    fs::read(directory.join(&manifest.library))?.hash(hash);
    let query_source = fs::read_to_string(directory.join(&manifest.query))?;
    query_source.hash(hash);
    // Installing an override explicitly authorizes loading this trusted native parser.
    let library = unsafe { libloading::Library::new(directory.join(&manifest.library)) }?;
    let language = unsafe {
        let symbol = format!("tree_sitter_{}", manifest.grammar);
        let load: libloading::Symbol<
            unsafe extern "C" fn() -> *const tree_sitter::ffi::TSLanguage,
        > = library
            .get(symbol.as_bytes())
            .map_err(|e| format!("missing parser export {symbol}: {e}"))?;
        let pointer = load();
        if pointer.is_null() {
            return Err(format!("{symbol} returned a null language").into());
        }
        // Tree-sitter's exported constructor returns a static language owned by the library.
        Language::from_raw(pointer)
    };
    Parser::new().set_language(&language)?;
    let query = Query::new(&language, &query_source)?;
    if !query.capture_names().contains(&"name")
        || !query
            .capture_names()
            .iter()
            .any(|n| n.starts_with("definition."))
    {
        return Err("symbol query must capture @name and @definition.<kind>".into());
    }
    for capture in query.capture_names() {
        if let Some(kind) = capture.strip_prefix("definition.")
            && ![
                "function",
                "method",
                "class",
                "interface",
                "type",
                "enum",
                "module",
                "constant",
                "struct",
                "trait",
                "event",
                "field",
                "heading",
                "macro",
            ]
            .contains(&kind)
            && !manifest
                .kind_overrides
                .iter()
                .any(|(cap, _, _)| cap == capture)
        {
            return Err(format!("unsupported symbol capture: @{capture}").into());
        }
    }
    for kind in &manifest.ref_node_types {
        if language.id_for_node_kind(kind, true) == 0 {
            return Err(format!("unknown reference node type: {kind}").into());
        }
    }
    for (capture, node, _) in &manifest.kind_overrides {
        if !query.capture_names().contains(&capture.as_str()) {
            return Err(format!("unknown kind override capture: {capture}").into());
        }
        if !node.is_empty() && language.id_for_node_kind(node, true) == 0 {
            return Err(format!("unknown kind override node: {node}").into());
        }
    }
    if let Some(body) = &manifest.sig_body_child
        && language.id_for_node_kind(body, true) == 0
        && language.field_id_for_name(body).is_none()
    {
        return Err(format!("unknown signature body node or field: {body}").into());
    }
    Ok(Override {
        config: LanguageConfig {
            name: manifest.name,
            extensions: manifest.extensions,
            grammar_override: Vec::new(),
            download_names: Vec::new(),
            query: query_source,
            sig_body_child: manifest.sig_body_child,
            sig_delimiter: manifest.sig_delimiter,
            kind_overrides: manifest.kind_overrides,
            ref_node_types: manifest.ref_node_types,
        },
        language,
        query,
        _library: library,
    })
}

pub fn install(name: &str, source: &Path) -> Result<(), Error> {
    if !valid_name(name) {
        return Err("invalid language name".into());
    }
    let source = fs::canonicalize(source)?;
    let mut manifest = read_manifest(&source)?;
    if manifest.name != name || !valid_name(&manifest.grammar) {
        return Err(
            "manifest name must match the requested language, and grammar must be a valid name"
                .into(),
        );
    }
    let _lock = registration_lock(true)?;
    let others: Vec<_> = read_registrations()?
        .into_iter()
        .filter(|(other, _)| other != name)
        .map(|(_, record)| read_manifest(&record.directory))
        .collect::<Result<_, _>>()?;
    check_extensions(
        name,
        &manifest.extensions,
        others.iter().map(|m| (&m.name, &m.extensions)),
    )?;

    let parsers = root().join("parsers");
    fs::create_dir_all(&parsers)?;
    let stage = tempfile::Builder::new()
        .prefix(&format!("{name}-"))
        .tempdir_in(parsers)?;
    let library = library_name(&manifest.grammar);
    fs::copy(source.join(&manifest.library), stage.path().join(&library))?;
    fs::copy(
        source.join(&manifest.query),
        stage.path().join("symbols.scm"),
    )?;
    manifest.library = library.into();
    manifest.query = "symbols.scm".into();
    fs::write(
        stage.path().join("language.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    // Validate the complete replacement before changing the active registration.
    drop(load(stage.path(), &mut DefaultHasher::new())?);
    let directory = fs::canonicalize(stage.path())?;
    let registration = Registration { directory, source };
    let mut file = tempfile::NamedTempFile::new_in(root())?;
    file.write_all(&serde_json::to_vec_pretty(&registration)?)?;
    file.persist(root().join(format!("{name}.json")))?;
    let _ = stage.keep();
    // ponytail: retain old snapshots for running processes (Windows locks DLLs).
    // Add explicit garbage collection if repeated installs make disk usage matter.
    Ok(())
}

pub fn remove(name: &str) -> Result<bool, Error> {
    if !valid_name(name) {
        return Err("invalid language name".into());
    }
    let _lock = registration_lock(true)?;
    match fs::remove_file(root().join(format!("{name}.json"))) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}
