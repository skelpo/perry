//! Compile command - compiles TypeScript to native executable

use anyhow::{anyhow, Result};
use clap::Args;
use perry_hir::{Module as HirModule, ModuleKind};
use perry_transform::inline_functions;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::OutputFormat;

#[derive(Args, Debug)]
pub struct CompileArgs {
    /// Input TypeScript file
    pub input: PathBuf,

    /// Output executable name
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Keep intermediate files (for debugging)
    #[arg(long)]
    pub keep_intermediates: bool,

    /// Print the HIR (for debugging)
    #[arg(long)]
    pub print_hir: bool,

    /// Don't link, just produce object file
    #[arg(long)]
    pub no_link: bool,

    /// Enable V8 JavaScript runtime for importing pure JS modules from node_modules.
    /// This is a fallback option when native compilation is not possible.
    /// WARNING: This significantly increases binary size (~10-15MB).
    #[arg(long)]
    pub enable_js_runtime: bool,
}

/// Information about a JavaScript module that will be interpreted at runtime
#[derive(Debug, Clone)]
pub struct JsModule {
    /// Absolute path to the JS file
    pub path: PathBuf,
    /// Source code of the JS module
    pub source: String,
    /// Module specifier used in imports (e.g., "lodash", "./utils.js")
    pub specifier: String,
}

/// Compilation context tracking all modules
#[derive(Debug)]
pub struct CompilationContext {
    /// Native TypeScript modules to compile
    pub native_modules: HashMap<PathBuf, HirModule>,
    /// JavaScript modules to interpret via V8
    pub js_modules: HashMap<String, JsModule>,
    /// Mapping from import specifiers to resolved paths
    pub import_map: HashMap<String, PathBuf>,
    /// Whether JS runtime is needed
    pub needs_js_runtime: bool,
    /// Project root (where we start looking for node_modules)
    pub project_root: PathBuf,
}

impl CompilationContext {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            native_modules: HashMap::new(),
            js_modules: HashMap::new(),
            import_map: HashMap::new(),
            needs_js_runtime: false,
            project_root,
        }
    }
}

/// Find the runtime library for linking
fn find_runtime_library() -> Result<PathBuf> {
    let candidates = [
        PathBuf::from("target/release/libperry_runtime.a"),
        PathBuf::from("target/debug/libperry_runtime.a"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("libperry_runtime.a")))
            .unwrap_or_default(),
        PathBuf::from("/usr/local/lib/libperry_runtime.a"),
    ];

    for path in &candidates {
        if path.exists() {
            return Ok(path.clone());
        }
    }

    Err(anyhow!(
        "Could not find libperry_runtime.a. Build it with: cargo build --release -p perry-runtime"
    ))
}

/// Find the stdlib library for linking (optional - only needed for native modules)
fn find_stdlib_library() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("target/release/libperry_stdlib.a"),
        PathBuf::from("target/debug/libperry_stdlib.a"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("libperry_stdlib.a")))
            .unwrap_or_default(),
        PathBuf::from("/usr/local/lib/libperry_stdlib.a"),
    ];

    for path in &candidates {
        if path.exists() {
            return Some(path.clone());
        }
    }

    None
}

/// Find the V8 jsruntime library for linking (optional - only needed for JS module support)
fn find_jsruntime_library() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("target/release/libperry_jsruntime.a"),
        PathBuf::from("target/debug/libperry_jsruntime.a"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("libperry_jsruntime.a")))
            .unwrap_or_default(),
        PathBuf::from("/usr/local/lib/libperry_jsruntime.a"),
    ];

    for path in &candidates {
        if path.exists() {
            return Some(path.clone());
        }
    }

    None
}

/// Find node_modules directory starting from a given path
fn find_node_modules(start: &Path) -> Option<PathBuf> {
    let mut current = start.to_path_buf();
    loop {
        let node_modules = current.join("node_modules");
        if node_modules.is_dir() {
            return Some(node_modules);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// Parse a package specifier into (package_name, subpath)
fn parse_package_specifier(specifier: &str) -> (String, Option<String>) {
    if specifier.starts_with('@') {
        // Scoped package: @scope/package or @scope/package/subpath
        let parts: Vec<&str> = specifier.splitn(3, '/').collect();
        if parts.len() >= 2 {
            let package_name = format!("{}/{}", parts[0], parts[1]);
            let subpath = if parts.len() > 2 {
                Some(parts[2].to_string())
            } else {
                None
            };
            return (package_name, subpath);
        }
    } else {
        // Regular package: package or package/subpath
        let parts: Vec<&str> = specifier.splitn(2, '/').collect();
        let package_name = parts[0].to_string();
        let subpath = if parts.len() > 1 {
            Some(parts[1].to_string())
        } else {
            None
        };
        return (package_name, subpath);
    }

    (specifier.to_string(), None)
}

/// Try to resolve a path with common extensions
/// Prefers TypeScript source files over JavaScript for native compilation
fn resolve_with_extensions(base: &Path) -> Option<PathBuf> {
    // TypeScript extensions to try (in order of preference)
    let ts_extensions = [".ts", ".tsx", ".mts"];
    // JavaScript extensions (fallback)
    let js_extensions = [".js", ".mjs", ".cjs"];
    // All extensions in order of preference
    let all_extensions = [".ts", ".tsx", ".mts", ".js", ".mjs", ".cjs", ".json"];

    // Check if the path has an explicit JS extension - if so, try TS equivalents first
    if let Some(ext) = base.extension().and_then(|e| e.to_str()) {
        if matches!(ext, "js" | "mjs" | "cjs") {
            // Strip the JS extension and try TS extensions first
            let stem = base.with_extension("");
            for ts_ext in ts_extensions {
                let ts_path = stem.with_extension(ts_ext.trim_start_matches('.'));
                if ts_path.exists() && ts_path.is_file() {
                    return Some(ts_path);
                }
            }
            // If no TS file found, fall back to the original JS file
            if base.exists() && base.is_file() {
                return Some(base.to_path_buf());
            }
        }
    }

    // If it already exists as-is (and not a JS file that we already handled above)
    if base.exists() && base.is_file() {
        // Even if it exists, check for TS version first
        if let Some(ext) = base.extension().and_then(|e| e.to_str()) {
            if matches!(ext, "js" | "mjs" | "cjs") {
                let stem = base.with_extension("");
                for ts_ext in ts_extensions {
                    let ts_path = stem.with_extension(ts_ext.trim_start_matches('.'));
                    if ts_path.exists() && ts_path.is_file() {
                        return Some(ts_path);
                    }
                }
            }
        }
        return Some(base.to_path_buf());
    }

    // Try with extensions in order of preference (TS before JS)
    for ext in all_extensions {
        let with_ext = base.with_extension(ext.trim_start_matches('.'));
        if with_ext.exists() && with_ext.is_file() {
            return Some(with_ext);
        }

        // Also try adding extension to full path (for paths like ./foo.js)
        let path_str = base.to_string_lossy();
        let with_ext = PathBuf::from(format!("{}{}", path_str, ext));
        if with_ext.exists() && with_ext.is_file() {
            // If we found a JS file, check for TS equivalent first
            if matches!(ext, ".js" | ".mjs" | ".cjs") {
                let stem_str = path_str.to_string();
                for ts_ext in ts_extensions {
                    let ts_path = PathBuf::from(format!("{}{}", stem_str, ts_ext));
                    if ts_path.exists() && ts_path.is_file() {
                        return Some(ts_path);
                    }
                }
            }
            return Some(with_ext);
        }
    }

    // Try index files in directory
    if base.is_dir() {
        for ext in all_extensions {
            let index = base.join(format!("index{}", ext));
            if index.exists() {
                return Some(index);
            }
        }
    }

    None
}

/// Resolve package.json entry point
fn resolve_package_entry(package_dir: &Path, subpath: Option<&str>) -> Option<PathBuf> {
    let package_json = package_dir.join("package.json");
    if !package_json.exists() {
        // Fall back to index.js
        return resolve_with_extensions(&package_dir.join("index"));
    }

    let content = fs::read_to_string(&package_json).ok()?;
    let pkg: serde_json::Value = serde_json::from_str(&content).ok()?;

    // Try "exports" field first (modern packages), for both main and subpaths
    let export_key = if let Some(sub) = subpath {
        format!("./{}", sub)
    } else {
        ".".to_string()
    };

    if let Some(exports) = pkg.get("exports") {
        if let Some(entry) = resolve_exports(exports, &export_key) {
            let entry_path = package_dir.join(&entry);
            if entry_path.exists() {
                return Some(entry_path);
            }
        }
    }

    // If there's a subpath and exports didn't match, resolve it directly
    if let Some(sub) = subpath {
        let subpath_resolved = package_dir.join(sub);
        return resolve_with_extensions(&subpath_resolved);
    }

    // Try "types" or "typings" field for TypeScript
    for field in ["types", "typings"] {
        if let Some(types_path) = pkg.get(field).and_then(|v| v.as_str()) {
            // Look for corresponding .ts file
            let types_file = package_dir.join(types_path);
            let ts_file = types_file.with_extension("ts");
            if ts_file.exists() {
                return Some(ts_file);
            }
        }
    }

    // Try "module" field (ESM)
    if let Some(module) = pkg.get("module").and_then(|v| v.as_str()) {
        let module_path = package_dir.join(module);
        if module_path.exists() {
            return Some(module_path);
        }
    }

    // Try "main" field (CommonJS)
    if let Some(main) = pkg.get("main").and_then(|v| v.as_str()) {
        let main_path = package_dir.join(main);
        return resolve_with_extensions(&main_path);
    }

    // Fall back to index files
    resolve_with_extensions(&package_dir.join("index"))
}

/// Resolve exports field from package.json
fn resolve_exports(exports: &serde_json::Value, subpath: &str) -> Option<String> {
    match exports {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => {
            // Try the specific subpath first
            if let Some(entry) = map.get(subpath) {
                return resolve_exports(entry, subpath);
            }

            // Try common conditions (for both main entry and subpath entries)
            // This handles the case where we've matched a subpath and now need to resolve the conditions
            for condition in ["import", "module", "default", "require", "node"] {
                if let Some(entry) = map.get(condition) {
                    return resolve_exports(entry, subpath);
                }
            }

            None
        }
        _ => None,
    }
}

/// Determine if a file is a JavaScript file (not TypeScript)
fn is_js_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(ext, "js" | "mjs" | "cjs")
    } else {
        false
    }
}

/// Determine if a file is a TypeScript declaration file (.d.ts)
fn is_declaration_file(path: &Path) -> bool {
    path.to_string_lossy().ends_with(".d.ts")
}

/// Determine if a file is a TypeScript file (but not a declaration file)
fn is_ts_file(path: &Path) -> bool {
    if is_declaration_file(path) {
        return false;
    }
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(ext, "ts" | "tsx")
    } else {
        false
    }
}

/// Resolve an import specifier to a file path
fn resolve_import(
    import_source: &str,
    importer_path: &Path,
    project_root: &Path,
) -> Option<(PathBuf, ModuleKind)> {
    // Check if it's a native Rust stdlib module
    if perry_hir::is_native_module(import_source) {
        return None; // Native modules are handled by stdlib, not file imports
    }

    // Handle relative imports (./ or ../)
    if import_source.starts_with("./") || import_source.starts_with("../") {
        let parent = importer_path.parent()?;
        let resolved = parent.join(import_source);
        if let Some(path) = resolve_with_extensions(&resolved) {
            let kind = if is_js_file(&path) {
                ModuleKind::Interpreted
            } else {
                ModuleKind::NativeCompiled
            };
            return Some((path.canonicalize().ok()?, kind));
        }
        return None;
    }

    // Handle absolute paths
    if import_source.starts_with('/') {
        let resolved = PathBuf::from(import_source);
        if let Some(path) = resolve_with_extensions(&resolved) {
            let kind = if is_js_file(&path) {
                ModuleKind::Interpreted
            } else {
                ModuleKind::NativeCompiled
            };
            return Some((path.canonicalize().ok()?, kind));
        }
        return None;
    }

    // Handle node_modules (bare specifiers)
    let (package_name, subpath) = parse_package_specifier(import_source);

    // Find node_modules starting from importer, then project root
    let search_paths = [importer_path.parent(), Some(project_root)];

    for start in search_paths.iter().flatten() {
        if let Some(node_modules) = find_node_modules(start) {
            let package_dir = node_modules.join(&package_name);
            if package_dir.is_dir() {
                if let Some(entry) = resolve_package_entry(&package_dir, subpath.as_deref()) {
                    // For node_modules packages, always treat as Interpreted
                    // Even .ts files in node_modules are library source code,
                    // not user code to be compiled. V8 will handle them at runtime.
                    return Some((entry.canonicalize().ok()?, ModuleKind::Interpreted));
                }
            }
        }
    }

    None
}

/// Collect all modules to compile (transitive closure of imports)
fn collect_modules(
    entry_path: &PathBuf,
    ctx: &mut CompilationContext,
    visited: &mut HashSet<PathBuf>,
    enable_js_runtime: bool,
    format: OutputFormat,
) -> Result<()> {
    let canonical = entry_path
        .canonicalize()
        .map_err(|e| anyhow!("Failed to canonicalize {}: {}", entry_path.display(), e))?;

    if visited.contains(&canonical) {
        return Ok(());
    }
    visited.insert(canonical.clone());

    // Check if this file should be handled by JS runtime instead of native compilation
    // This includes: JS files, declaration files (.d.ts), or any file in node_modules when JS runtime is enabled
    let is_in_node_modules = canonical.to_string_lossy().contains("node_modules");
    let should_use_js_runtime = is_js_file(&canonical)
        || is_declaration_file(&canonical)
        || (enable_js_runtime && is_in_node_modules);

    if should_use_js_runtime {
        if !enable_js_runtime {
            return Err(anyhow!(
                "File '{}' requires --enable-js-runtime flag",
                canonical.display()
            ));
        }

        // Skip declaration files - they're just type information
        if is_declaration_file(&canonical) {
            return Ok(());
        }

        let source = fs::read_to_string(&canonical)
            .map_err(|e| anyhow!("Failed to read {}: {}", canonical.display(), e))?;

        let specifier = canonical.to_string_lossy().to_string();
        ctx.js_modules.insert(specifier.clone(), JsModule {
            path: canonical.clone(),
            source,
            specifier,
        });
        ctx.needs_js_runtime = true;

        // We don't parse JS/node_modules files for their imports (V8 will handle that at runtime)
        return Ok(());
    }

    // It's a TypeScript file to compile natively
    let source = fs::read_to_string(&canonical)
        .map_err(|e| anyhow!("Failed to read {}: {}", canonical.display(), e))?;

    let filename = canonical
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("input.ts");

    // Use a relative path from project root for unique module names
    // This ensures files like "routes/auth.ts" and "middleware/auth.ts" have different names
    let module_name = canonical
        .strip_prefix(&ctx.project_root)
        .ok()
        .and_then(|p| p.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| filename.to_string());

    let ast_module = perry_parser::parse_typescript(&source, filename)?;
    let source_file_path = canonical.to_string_lossy().to_string();
    let mut hir_module = perry_hir::lower_module(&ast_module, &module_name, &source_file_path)?;

    // Apply function inlining optimization
    inline_functions(&mut hir_module);

    // Process imports and update their resolved paths and module kinds
    for import in &mut hir_module.imports {
        if import.is_native {
            import.module_kind = ModuleKind::NativeRust;
            continue;
        }

        if let Some((resolved_path, kind)) = resolve_import(&import.source, &canonical, &ctx.project_root) {
            import.resolved_path = Some(resolved_path.to_string_lossy().to_string());
            import.module_kind = kind;

            match kind {
                ModuleKind::NativeCompiled => {
                    // Recursively collect TypeScript modules
                    collect_modules(&resolved_path, ctx, visited, enable_js_runtime, format)?;
                }
                ModuleKind::Interpreted => {
                    // Skip declaration files (.d.ts) - they only contain type information
                    if is_declaration_file(&resolved_path) {
                        continue;
                    }

                    if !enable_js_runtime {
                        return Err(anyhow!(
                            "Import '{}' resolves to JavaScript file '{}' which requires --enable-js-runtime flag",
                            import.source,
                            resolved_path.display()
                        ));
                    }

                    match format {
                        OutputFormat::Text => {
                            println!("  JS module: {} -> {}", import.source, resolved_path.display());
                        }
                        OutputFormat::Json => {}
                    }

                    // Collect JS module
                    collect_modules(&resolved_path, ctx, visited, enable_js_runtime, format)?;
                }
                ModuleKind::NativeRust => {
                    // Native Rust modules are handled by stdlib
                }
            }
        } else {
            // Could not resolve - might be a Node.js builtin or missing module
            // For now, treat unresolved non-native imports as errors
            if !import.is_native {
                match format {
                    OutputFormat::Text => {
                        println!("  Warning: Could not resolve import '{}' from {}", import.source, filename);
                    }
                    OutputFormat::Json => {}
                }
            }
        }
    }

    // Process re-exports
    for export in &hir_module.exports {
        let source = match export {
            perry_hir::Export::ReExport { source, .. } => Some(source),
            perry_hir::Export::ExportAll { source } => Some(source),
            perry_hir::Export::Named { .. } => None,
        };
        if let Some(src) = source {
            if let Some((resolved_path, kind)) = resolve_import(src, &canonical, &ctx.project_root) {
                match kind {
                    ModuleKind::NativeCompiled => {
                        collect_modules(&resolved_path, ctx, visited, enable_js_runtime, format)?;
                    }
                    ModuleKind::Interpreted => {
                        if enable_js_runtime {
                            collect_modules(&resolved_path, ctx, visited, enable_js_runtime, format)?;
                        }
                    }
                    ModuleKind::NativeRust => {}
                }
            }
        }
    }

    ctx.native_modules.insert(canonical, hir_module);
    Ok(())
}

/// Generate a JS bundle file containing all JS modules
fn generate_js_bundle(ctx: &CompilationContext, output_dir: &Path) -> Result<PathBuf> {
    let bundle_path = output_dir.join("__perry_js_bundle.js");

    let mut bundle = String::new();
    bundle.push_str("// Auto-generated JS bundle by Perry\n");
    bundle.push_str("// This file contains all JavaScript modules needed at runtime\n\n");

    bundle.push_str("globalThis.__COMPILETS_MODULES = {};\n\n");

    for (specifier, module) in &ctx.js_modules {
        // Escape the source code for embedding
        let escaped_source = module.source
            .replace('\\', "\\\\")
            .replace('`', "\\`")
            .replace("${", "\\${");

        bundle.push_str(&format!(
            "globalThis.__COMPILETS_MODULES[{:?}] = `{}`;\n",
            specifier, escaped_source
        ));
    }

    fs::write(&bundle_path, &bundle)?;
    Ok(bundle_path)
}

pub fn run(args: CompileArgs, format: OutputFormat, _use_color: bool, _verbose: u8) -> Result<()> {
    match format {
        OutputFormat::Text => println!("Collecting modules..."),
        OutputFormat::Json => {}
    }

    let project_root = args.input
        .parent()
        .unwrap_or(Path::new("."))
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."));

    let mut ctx = CompilationContext::new(project_root);
    let mut visited = HashSet::new();

    collect_modules(&args.input, &mut ctx, &mut visited, args.enable_js_runtime, format)?;

    let total_modules = ctx.native_modules.len() + ctx.js_modules.len();
    match format {
        OutputFormat::Text => {
            println!("Found {} module(s): {} native, {} JavaScript",
                total_modules,
                ctx.native_modules.len(),
                ctx.js_modules.len()
            );
        }
        OutputFormat::Json => {}
    }

    // Transform JS imports into runtime calls
    if ctx.needs_js_runtime {
        for (_, hir_module) in ctx.native_modules.iter_mut() {
            perry_hir::transform_js_imports(hir_module);
        }
    }

    // Build map of exported native instances from all modules
    let mut exported_instances: HashMap<(String, String), perry_hir::ExportedNativeInstance> = HashMap::new();
    for (path, hir_module) in &ctx.native_modules {
        let path_str = path.to_string_lossy().to_string();
        for (export_name, native_module, native_class) in &hir_module.exported_native_instances {
            exported_instances.insert(
                (path_str.clone(), export_name.clone()),
                perry_hir::ExportedNativeInstance {
                    native_module: native_module.clone(),
                    native_class: native_class.clone(),
                },
            );
        }
    }

    // Fix cross-module native instance method calls
    if !exported_instances.is_empty() {
        for (_, hir_module) in ctx.native_modules.iter_mut() {
            perry_hir::fix_cross_module_native_instances(hir_module, &exported_instances);
        }
    }

    // Run monomorphization pass on all native modules
    for (_, hir_module) in ctx.native_modules.iter_mut() {
        perry_hir::monomorphize_module(hir_module);
    }

    if args.print_hir {
        for (path, hir_module) in &ctx.native_modules {
            println!("\n=== HIR (after monomorphization): {} ===", path.display());
            println!("Module: {}", hir_module.name);
            println!("Imports: {}", hir_module.imports.len());
            for import in &hir_module.imports {
                println!(
                    "  - {} ({} specifiers, kind: {:?})",
                    import.source,
                    import.specifiers.len(),
                    import.module_kind
                );
            }
            println!("Exports: {}", hir_module.exports.len());
            println!("Functions: {}", hir_module.functions.len());
            for func in &hir_module.functions {
                println!(
                    "  - {} (params: {}, type_params: {}, async: {}, exported: {})",
                    func.name,
                    func.params.len(),
                    func.type_params.len(),
                    func.is_async,
                    func.is_exported
                );
            }
            println!("Init statements: {}", hir_module.init.len());
            for (i, stmt) in hir_module.init.iter().enumerate() {
                println!("  [{}] {:?}", i, stmt);
            }
            println!("===========\n");
        }

        if !ctx.js_modules.is_empty() {
            println!("\n=== JavaScript Modules (interpreted) ===");
            for (specifier, module) in &ctx.js_modules {
                println!("  {} -> {}", specifier, module.path.display());
            }
            println!("===========\n");
        }
    }

    match format {
        OutputFormat::Text => println!("Generating code..."),
        OutputFormat::Json => {}
    }

    let mut obj_paths = Vec::new();

    // Get canonical path of entry module
    let entry_path = args.input.canonicalize().unwrap_or_else(|_| args.input.clone());

    // Collect non-entry module names for init function calls
    let non_entry_module_names: Vec<String> = ctx.native_modules
        .iter()
        .filter(|(path, _)| *path != &entry_path)
        .map(|(_, hir_module)| hir_module.name.clone())
        .collect();

    // Build a map of all exported classes from all modules
    // Key: (resolved_path, class_name) -> Class reference
    let mut exported_classes: HashMap<(String, String), &perry_hir::Class> = HashMap::new();
    for (path, hir_module) in &ctx.native_modules {
        let path_str = path.to_string_lossy().to_string();
        for class in &hir_module.classes {
            if class.is_exported {
                exported_classes.insert((path_str.clone(), class.name.clone()), class);
            }
        }
    }

    // Propagate re-exports: when module A has `export * from "./B"`, all classes
    // exported from B should also be accessible via A's path.
    // We need to iterate until no new entries are added (for chained re-exports).
    loop {
        let mut new_entries: Vec<((String, String), &perry_hir::Class)> = Vec::new();

        for (path, hir_module) in &ctx.native_modules {
            let path_str = path.to_string_lossy().to_string();

            for export in &hir_module.exports {
                if let perry_hir::Export::ExportAll { source } = export {
                    // Resolve the source path relative to this module
                    if let Some((resolved_source, _)) = resolve_import(source, path, &ctx.project_root) {
                        let source_path_str = resolved_source.to_string_lossy().to_string();

                        // Find all classes exported from the source module and add them
                        // as also being exported from this module
                        for ((src_path, class_name), class) in &exported_classes {
                            if src_path == &source_path_str {
                                let key = (path_str.clone(), class_name.clone());
                                if !exported_classes.contains_key(&key) {
                                    new_entries.push((key, *class));
                                }
                            }
                        }
                    }
                }
            }
        }

        if new_entries.is_empty() {
            break;
        }

        for (key, class) in new_entries {
            exported_classes.insert(key, class);
        }
    }

    // Compile native modules
    for (path, hir_module) in &ctx.native_modules {
        let mut compiler = perry_codegen::Compiler::new()?;

        // Check if this is the entry module
        let is_entry = path == &entry_path;
        compiler.set_is_entry_module(is_entry);

        // For entry module, add init function calls for all other native modules
        if is_entry {
            for module_name in &non_entry_module_names {
                compiler.add_native_module_init(module_name.clone());
            }
        }

        // If we need JS runtime, tell the compiler to generate init code
        if ctx.needs_js_runtime {
            compiler.set_needs_js_runtime(true);
            // Pass JS module paths for loading
            for (specifier, _module) in &ctx.js_modules {
                compiler.add_js_module(specifier.clone());
            }
        }

        // Register imported classes from other native modules
        for import in &hir_module.imports {
            // Only process imports from other native TypeScript modules
            if import.module_kind != perry_hir::ModuleKind::NativeCompiled {
                continue;
            }

            let resolved_path = match &import.resolved_path {
                Some(p) => p.clone(),
                None => continue,
            };

            for spec in &import.specifiers {
                let (local_name, exported_name) = match spec {
                    perry_hir::ImportSpecifier::Named { imported, local } => (local.clone(), imported.clone()),
                    perry_hir::ImportSpecifier::Default { local } => (local.clone(), local.clone()),
                    perry_hir::ImportSpecifier::Namespace { .. } => continue,
                };

                // Check if this import is a class from another module
                let key = (resolved_path.clone(), exported_name);
                if let Some(class) = exported_classes.get(&key) {
                    // Register this class as an import in the current compiler
                    // Pass the local_name as an alias so the class can be found when used with that name
                    compiler.register_imported_class(class, Some(&local_name))?;
                }
            }
        }

        let object_code = compiler.compile_module(hir_module)?;

        // Generate a unique object file name to handle files with same basename in different directories
        // e.g., routes/auth.ts -> routes_auth.o, middleware/auth.ts -> middleware_auth.o
        let obj_name = {
            // Try to get a unique name by including parent directory
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("module");
            let expected_obj_name = format!("{}.o", stem);
            if let Some(parent) = path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()) {
                // Check if there might be a conflict (another file with same stem exists)
                let simple_obj = PathBuf::from(&expected_obj_name);
                let has_conflict = simple_obj.exists() || obj_paths.iter().any(|p: &PathBuf| {
                    p.file_name().and_then(|s: &std::ffi::OsStr| s.to_str()) == Some(&expected_obj_name)
                });
                if has_conflict {
                    format!("{}_{}", parent, stem)
                } else {
                    stem.to_string()
                }
            } else {
                stem.to_string()
            }
        };
        let obj_path = PathBuf::from(format!("{}.o", obj_name));

        fs::write(&obj_path, &object_code)?;
        match format {
            OutputFormat::Text => println!("Wrote object file: {}", obj_path.display()),
            OutputFormat::Json => {}
        }
        obj_paths.push(obj_path);
    }

    // Generate JS bundle if needed
    let _js_bundle_path = if ctx.needs_js_runtime && !ctx.js_modules.is_empty() {
        let bundle_path = generate_js_bundle(&ctx, Path::new("."))?;
        match format {
            OutputFormat::Text => println!("Generated JS bundle: {}", bundle_path.display()),
            OutputFormat::Json => {}
        }
        Some(bundle_path)
    } else {
        None
    };

    let stem = args
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let exe_path = args.output.unwrap_or_else(|| PathBuf::from(stem));

    if args.no_link {
        return Ok(());
    }

    match format {
        OutputFormat::Text => println!("Linking..."),
        OutputFormat::Json => {}
    }

    let runtime_lib = find_runtime_library()?;
    let stdlib_lib = find_stdlib_library();
    let jsruntime_lib = if ctx.needs_js_runtime || args.enable_js_runtime {
        match find_jsruntime_library() {
            Some(lib) => {
                match format {
                    OutputFormat::Text => println!("Using V8 JavaScript runtime for JS module support"),
                    OutputFormat::Json => {}
                }
                Some(lib)
            }
            None => {
                if ctx.needs_js_runtime {
                    return Err(anyhow!(
                        "JavaScript modules found but libperry_jsruntime.a not found. Build it with: cargo build --release -p perry-jsruntime"
                    ));
                }
                None
            }
        }
    } else {
        None
    };

    let mut cmd = Command::new("cc");
    for obj_path in &obj_paths {
        cmd.arg(obj_path);
    }
    cmd.arg(&runtime_lib);

    // Add stdlib library if available (needed for native module support like mysql2)
    if let Some(ref stdlib) = stdlib_lib {
        cmd.arg(stdlib);
    }

    // Add jsruntime library if V8 support is enabled
    if let Some(ref jsruntime) = jsruntime_lib {
        cmd.arg(jsruntime);
    }

    cmd.arg("-o")
        .arg(&exe_path)
        .arg("-lc");

    // On macOS, we need additional frameworks for tokio/networking/sqlx/V8
    #[cfg(target_os = "macos")]
    {
        if stdlib_lib.is_some() || jsruntime_lib.is_some() {
            cmd.arg("-framework").arg("Security")
               .arg("-framework").arg("CoreFoundation")
               .arg("-framework").arg("SystemConfiguration")
               .arg("-framework").arg("IOKit")
               .arg("-liconv")
               .arg("-lresolv");
        }

        // V8 requires additional C++ runtime
        if jsruntime_lib.is_some() {
            cmd.arg("-lc++");
        }
    }

    // On Linux, link against pthread and dl for V8
    #[cfg(target_os = "linux")]
    {
        if jsruntime_lib.is_some() {
            cmd.arg("-lpthread")
               .arg("-ldl")
               .arg("-lstdc++");
        }
    }

    let status = cmd.status()?;

    if !status.success() {
        return Err(anyhow!("Linking failed"));
    }

    match format {
        OutputFormat::Text => println!("Wrote executable: {}", exe_path.display()),
        OutputFormat::Json => {
            let result = serde_json::json!({
                "success": true,
                "output": exe_path.to_string_lossy(),
                "native_modules": ctx.native_modules.len(),
                "js_modules": ctx.js_modules.len(),
            });
            println!("{}", serde_json::to_string(&result)?);
        }
    }

    if !args.keep_intermediates {
        for obj_path in &obj_paths {
            let _ = fs::remove_file(obj_path);
        }
    }

    Ok(())
}
