//! Module loader for V8 runtime
//!
//! Handles loading JavaScript modules from node_modules and local paths.

use anyhow::{anyhow, Result};
use deno_core::error::AnyError;
use deno_core::{ModuleLoadResponse, ModuleLoader, ModuleSource, ModuleSourceCode, ModuleSpecifier, ModuleType, RequestedModuleType, ResolutionKind};
use std::path::{Path, PathBuf};

/// Node.js-compatible module loader
pub struct NodeModuleLoader {
    /// Base directory for module resolution
    base_dir: PathBuf,
}

impl NodeModuleLoader {
    pub fn new() -> Self {
        Self {
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Resolve a module specifier to an absolute path
    fn resolve_module_path(&self, specifier: &str, referrer: &Path) -> Result<PathBuf> {
        // Handle file:// URLs
        if specifier.starts_with("file://") {
            let path_str = specifier.strip_prefix("file://").unwrap_or(specifier);
            let path = PathBuf::from(path_str);
            if path.exists() {
                return Ok(path);
            }
            return self.resolve_with_extensions(path);
        }

        // Handle relative imports (./ or ../)
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let referrer_dir = referrer.parent().unwrap_or(&self.base_dir);
            let resolved = referrer_dir.join(specifier);
            return self.resolve_with_extensions(resolved);
        }

        // Handle absolute paths
        if specifier.starts_with('/') {
            let resolved = PathBuf::from(specifier);
            return self.resolve_with_extensions(resolved);
        }

        // Handle node_modules
        self.resolve_from_node_modules(specifier, referrer)
    }

    /// Try resolving a path with common extensions
    fn resolve_with_extensions(&self, base: PathBuf) -> Result<PathBuf> {
        // If it already exists as-is
        if base.exists() && base.is_file() {
            return Ok(base);
        }

        // Try with extensions
        let extensions = [".js", ".mjs", ".cjs", ".json"];
        for ext in extensions {
            let with_ext = base.with_extension(ext.trim_start_matches('.'));
            if with_ext.exists() {
                return Ok(with_ext);
            }

            // Also try adding extension to full path (for paths like ./foo.js)
            let path_str = base.to_string_lossy();
            let with_ext = PathBuf::from(format!("{}{}", path_str, ext));
            if with_ext.exists() {
                return Ok(with_ext);
            }
        }

        // Try index files in directory
        if base.is_dir() {
            for ext in extensions {
                let index = base.join(format!("index{}", ext));
                if index.exists() {
                    return Ok(index);
                }
            }
        }

        Err(anyhow!("Cannot resolve module: {:?}", base))
    }

    /// Resolve a module from node_modules
    fn resolve_from_node_modules(&self, specifier: &str, referrer: &Path) -> Result<PathBuf> {
        let mut current_dir = referrer.parent().unwrap_or(&self.base_dir).to_path_buf();

        // Parse package name and subpath
        let (package_name, subpath) = parse_package_specifier(specifier);

        // Walk up the directory tree looking for node_modules
        loop {
            let node_modules = current_dir.join("node_modules").join(&package_name);

            if node_modules.exists() {
                // Check for package.json
                let package_json = node_modules.join("package.json");
                if package_json.exists() {
                    if let Ok(entry_point) = self.resolve_package_entry(&node_modules, &package_json, subpath.as_deref()) {
                        return Ok(entry_point);
                    }
                }

                // Fall back to index.js
                let index = node_modules.join("index.js");
                if index.exists() {
                    return Ok(index);
                }
            }

            // Move up to parent directory
            if let Some(parent) = current_dir.parent() {
                current_dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        Err(anyhow!(
            "Cannot find module '{}' in node_modules",
            specifier
        ))
    }

    /// Resolve the entry point from package.json
    fn resolve_package_entry(
        &self,
        package_dir: &Path,
        package_json: &Path,
        subpath: Option<&str>,
    ) -> Result<PathBuf> {
        let content = std::fs::read_to_string(package_json)?;
        let pkg: serde_json::Value = serde_json::from_str(&content)?;

        // If there's a subpath, resolve it directly
        if let Some(sub) = subpath {
            let subpath_resolved = package_dir.join(sub);
            return self.resolve_with_extensions(subpath_resolved);
        }

        // Try "exports" field first (modern packages)
        if let Some(exports) = pkg.get("exports") {
            if let Some(entry) = resolve_exports(exports, ".") {
                let entry_path = package_dir.join(entry);
                if entry_path.exists() {
                    return Ok(entry_path);
                }
            }
        }

        // Try "module" field (ESM)
        if let Some(module) = pkg.get("module").and_then(|v| v.as_str()) {
            let module_path = package_dir.join(module);
            if module_path.exists() {
                return Ok(module_path);
            }
        }

        // Try "main" field (CommonJS)
        if let Some(main) = pkg.get("main").and_then(|v| v.as_str()) {
            let main_path = package_dir.join(main);
            return self.resolve_with_extensions(main_path);
        }

        // Fall back to index.js
        let index = package_dir.join("index.js");
        if index.exists() {
            return Ok(index);
        }

        Err(anyhow!("Cannot resolve package entry point"))
    }

    /// Detect if a file is CommonJS or ESM
    fn detect_module_type(&self, path: &Path) -> ModuleType {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match extension {
            "mjs" => ModuleType::JavaScript,
            "cjs" => ModuleType::JavaScript, // Will be wrapped as CommonJS
            "json" => ModuleType::Json,
            _ => {
                // Check package.json for "type": "module"
                if let Some(parent) = path.parent() {
                    let package_json = parent.join("package.json");
                    if package_json.exists() {
                        if let Ok(content) = std::fs::read_to_string(&package_json) {
                            if let Ok(pkg) = serde_json::from_str::<serde_json::Value>(&content) {
                                if pkg.get("type").and_then(|v| v.as_str()) == Some("module") {
                                    return ModuleType::JavaScript;
                                }
                            }
                        }
                    }
                }
                ModuleType::JavaScript
            }
        }
    }
}

impl Default for NodeModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleLoader for NodeModuleLoader {
    fn resolve(
        &self,
        specifier: &str,
        referrer: &str,
        _kind: ResolutionKind,
    ) -> Result<ModuleSpecifier, AnyError> {
        let referrer_path = if referrer.starts_with("file://") {
            PathBuf::from(referrer.strip_prefix("file://").unwrap_or(referrer))
        } else {
            PathBuf::from(referrer)
        };

        let resolved_path = self.resolve_module_path(specifier, &referrer_path)?;

        let canonical = std::fs::canonicalize(&resolved_path)
            .unwrap_or(resolved_path);

        ModuleSpecifier::from_file_path(&canonical)
            .map_err(|_| anyhow!("Failed to create module specifier for {:?}", canonical).into())
    }

    fn load(
        &self,
        module_specifier: &ModuleSpecifier,
        _maybe_referrer: Option<&ModuleSpecifier>,
        _is_dyn_import: bool,
        _requested_module_type: RequestedModuleType,
    ) -> ModuleLoadResponse {
        let path = match module_specifier.to_file_path() {
            Ok(p) => p,
            Err(_) => return ModuleLoadResponse::Sync(Err(anyhow!("Invalid file path").into())),
        };

        let code = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => return ModuleLoadResponse::Sync(Err(anyhow!("Failed to read module: {}", e).into())),
        };

        let module_type = self.detect_module_type(&path);

        // Wrap CommonJS modules if needed
        let code = if is_commonjs(&code) {
            wrap_commonjs(&code)
        } else {
            code
        };

        ModuleLoadResponse::Sync(Ok(ModuleSource::new(
            module_type,
            ModuleSourceCode::String(code.into()),
            module_specifier,
            None,
        )))
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

/// Resolve exports field from package.json
fn resolve_exports(exports: &serde_json::Value, subpath: &str) -> Option<String> {
    match exports {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => {
            // Try the specific subpath
            if let Some(entry) = map.get(subpath) {
                return resolve_exports(entry, subpath);
            }

            // Try "." for main entry
            if subpath == "." {
                // Try common conditions
                for condition in ["import", "module", "default", "require", "node"] {
                    if let Some(entry) = map.get(condition) {
                        return resolve_exports(entry, subpath);
                    }
                }
            }

            None
        }
        _ => None,
    }
}

/// Check if code appears to be CommonJS
fn is_commonjs(code: &str) -> bool {
    // Quick heuristics for CommonJS detection
    code.contains("module.exports")
        || code.contains("exports.")
        || (code.contains("require(") && !code.contains("import "))
}

/// Wrap CommonJS code as ESM
fn wrap_commonjs(code: &str) -> String {
    format!(
        r#"
const module = {{ exports: {{}} }};
const exports = module.exports;
function require(specifier) {{
    throw new Error('require() is not supported. Use ESM imports instead.');
}}

{}

export default module.exports;
const __exports = module.exports;
export {{ __exports as exports }};
"#,
        code
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_specifier() {
        assert_eq!(
            parse_package_specifier("lodash"),
            ("lodash".to_string(), None)
        );
        assert_eq!(
            parse_package_specifier("lodash/map"),
            ("lodash".to_string(), Some("map".to_string()))
        );
        assert_eq!(
            parse_package_specifier("@types/node"),
            ("@types/node".to_string(), None)
        );
        assert_eq!(
            parse_package_specifier("@babel/core/lib/parser"),
            ("@babel/core".to_string(), Some("lib/parser".to_string()))
        );
    }

    #[test]
    fn test_is_commonjs() {
        assert!(is_commonjs("module.exports = {};"));
        assert!(is_commonjs("exports.foo = 'bar';"));
        assert!(!is_commonjs("export default {};"));
        assert!(!is_commonjs("import foo from 'bar';"));
    }
}
