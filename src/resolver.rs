use std::path::{Path, PathBuf};

/// Get the src directory from a given path
pub fn get_src_dir(path: &Path) -> PathBuf {
    let mut current = path;
    while let Some(parent) = current.parent() {
        if parent.ends_with("src") {
            return parent.to_path_buf();
        }
        current = parent;
    }
    // Fallback: assume current directory has src
    std::env::current_dir()
        .unwrap_or_default()
        .join("src")
}

/// Find a module file by navigating through module path components
pub fn find_module_by_path(src_dir: &Path, module_path: &[String]) -> Option<PathBuf> {
    if module_path.is_empty() {
        return None;
    }

    // Start from src_dir
    let mut current_dir = src_dir.to_path_buf();

    // Navigate through each component
    for (index, module_name) in module_path.iter().enumerate() {
        let is_last = index == module_path.len() - 1;

        // Try module_name/mod.rs
        let mod_dir = current_dir.join(module_name);
        let mod_path = mod_dir.join("mod.rs");
        if mod_path.exists() {
            if is_last {
                return Some(mod_path);
            }
            // Continue into this module's directory
            current_dir = mod_dir;
            continue;
        }

        // Try module_name.rs
        let file_path = current_dir.join(format!("{}.rs", module_name));
        if file_path.exists() {
            if is_last {
                return Some(file_path);
            }
            // For non-last components, need to check if there's a directory with the same name
            let module_dir = current_dir.join(module_name);
            if module_dir.is_dir() {
                current_dir = module_dir;
                continue;
            } else {
                // No directory to continue into
                return None;
            }
        }

        // Module not found
        return None;
    }

    None
}

/// Find a submodule file relative to a parent module file
pub fn find_submodule(parent_path: &Path, submodule_name: &str) -> Option<PathBuf> {
    let parent_dir = parent_path.parent()?;

    // If parent is mod.rs, look in the same directory
    if parent_path.file_name()? == "mod.rs" {
        let base_dir = parent_dir;

        // Check for submodule_name.rs in same directory
        let file_path = base_dir.join(format!("{}.rs", submodule_name));
        if file_path.exists() {
            return Some(file_path);
        }

        // Check for submodule_name/mod.rs
        let mod_path = base_dir.join(submodule_name).join("mod.rs");
        if mod_path.exists() {
            return Some(mod_path);
        }
    } else {
        // Parent is a regular file (e.g., module.rs)
        let module_name = parent_path.file_stem()?.to_str()?;
        let module_dir = parent_dir.join(module_name);

        // Check for module_name/submodule_name.rs
        let file_path = module_dir.join(format!("{}.rs", submodule_name));
        if file_path.exists() {
            return Some(file_path);
        }

        // Check for module_name/submodule_name/mod.rs
        let mod_path = module_dir.join(submodule_name).join("mod.rs");
        if mod_path.exists() {
            return Some(mod_path);
        }
    }

    None
}

/// Resolve a module path (e.g., ["crate", "foo", "bar"]) to a file system path
pub fn resolve_module_path_to_file(
    src_dir: &Path,
    module_path: &[String],
    verbose: bool,
) -> Option<PathBuf> {
    if module_path.is_empty() {
        if verbose {
            eprintln!("Debug: Module path is empty");
        }
        return None;
    }

    // First element should be "crate" for internal uses
    if module_path[0] != "crate" {
        if verbose {
            eprintln!(
                "Debug: Module path doesn't start with 'crate': {:?}",
                module_path
            );
        }
        return None;
    }

    // Start from src_dir
    let mut current_path = src_dir.to_path_buf();

    if verbose {
        eprintln!("Debug: Starting from src_dir: {}", current_path.display());
    }

    // Navigate through the module path (skip "crate" at index 0)
    for (idx, module_name) in module_path[1..].iter().enumerate() {
        let is_last = idx == module_path.len() - 2; // -2 because we skip "crate" at index 0

        if verbose {
            eprintln!(
                "Debug: Looking for module '{}' in {} (is_last={})",
                module_name,
                current_path.display(),
                is_last
            );
        }

        // Try module_name/mod.rs
        let mod_dir = current_path.join(module_name);
        let mod_path = mod_dir.join("mod.rs");
        if mod_path.exists() {
            if verbose {
                eprintln!("Debug: Found {}", mod_path.display());
            }
            if is_last {
                // This is the final module, return the mod.rs file
                current_path = mod_path;
            } else {
                // Not the final module, continue in the module directory
                current_path = mod_dir;
            }
            continue;
        }

        // Try module_name.rs
        let file_path = current_path.join(format!("{}.rs", module_name));
        if file_path.exists() {
            if verbose {
                eprintln!("Debug: Found {}", file_path.display());
            }
            if is_last {
                // This is the final module, return the .rs file
                current_path = file_path;
            } else {
                // Not the final module, need to navigate into module_name/ directory
                let module_dir = current_path.join(module_name);
                if module_dir.is_dir() {
                    if verbose {
                        eprintln!(
                            "Debug: Navigating into companion directory {}",
                            module_dir.display()
                        );
                    }
                    current_path = module_dir;
                } else {
                    if verbose {
                        eprintln!(
                            "Debug: No companion directory found for {}",
                            file_path.display()
                        );
                    }
                    return None;
                }
            }
            continue;
        }

        // Module not found
        if verbose {
            eprintln!("Debug: Module '{}' not found at index {}", module_name, idx);
        }
        return None;
    }

    // If current_path is a directory, look for mod.rs
    if current_path.is_dir() {
        let mod_path = current_path.join("mod.rs");
        if mod_path.exists() {
            if verbose {
                eprintln!(
                    "Debug: Final path is directory, using mod.rs: {}",
                    mod_path.display()
                );
            }
            return Some(mod_path);
        }
    }

    if current_path.is_file() {
        if verbose {
            eprintln!("Debug: Final resolved file: {}", current_path.display());
        }
        Some(current_path)
    } else {
        if verbose {
            eprintln!(
                "Debug: Final path is not a file: {}",
                current_path.display()
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_src_dir() {
        let path = PathBuf::from("/home/user/project/src/module.rs");
        let src_dir = get_src_dir(&path);
        assert!(src_dir.ends_with("src"));
    }
}
