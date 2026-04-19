//! Tests that verify the SDK does not depend on any web framework crates.
//!
//! These are static/structural checks that scan Cargo.toml and source files
//! to ensure the SDK remains a lightweight client library.

use std::fs;
use std::path::{Path, PathBuf};

/// Web framework crates that must NOT appear as dependencies.
const BANNED_FRAMEWORK_CRATES: &[&str] = &[
    "actix-web",
    "axum",
    "rocket",
    "warp",
    "tide",
    "poem",
];

/// Allowed dependency crates (HTTP client, serialization, async runtime).
const ALLOWED_CRATES: &[&str] = &[
    "reqwest",
    "ureq",
    "serde",
    "serde_json",
    "tokio",
];

fn project_root() -> PathBuf {
    // tests/ is one level below the project root
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

// -------------------------------------------------------------------------
// Static checks: scan source files for banned framework imports
// -------------------------------------------------------------------------

#[test]
fn source_files_do_not_import_web_framework_crates() {
    let src_dir = project_root().join("src");
    assert!(src_dir.exists(), "src/ directory should exist");

    let mut violations: Vec<(String, usize, String)> = Vec::new();

    for entry in fs::read_dir(&src_dir).expect("failed to read src/") {
        let entry = entry.expect("failed to read dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        for (line_num, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with("//") {
                continue;
            }

            for &framework in BANNED_FRAMEWORK_CRATES {
                // Convert crate name to Rust module form (e.g. "actix-web" -> "actix_web")
                let rust_name = framework.replace('-', "_");

                // Check for `use framework_name` or `extern crate framework_name`
                if trimmed.starts_with(&format!("use {rust_name}"))
                    || trimmed.starts_with(&format!("extern crate {rust_name}"))
                    || trimmed.contains(&format!("{rust_name}::"))
                {
                    violations.push((
                        path.file_name().unwrap().to_string_lossy().to_string(),
                        line_num + 1,
                        format!("references banned framework '{framework}': {trimmed}"),
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Found web framework references in source files:\n{}",
        violations
            .iter()
            .map(|(file, line, msg)| format!("  {file}:{line}: {msg}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn source_files_only_use_allowed_external_crates() {
    let src_dir = project_root().join("src");

    let mut violations: Vec<(String, usize, String)> = Vec::new();

    for entry in fs::read_dir(&src_dir).expect("failed to read src/") {
        let entry = entry.expect("failed to read dir entry");
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }

        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        for (line_num, line) in contents.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                continue;
            }

            for &framework in BANNED_FRAMEWORK_CRATES {
                let rust_name = framework.replace('-', "_");
                if trimmed.starts_with(&format!("use {rust_name}"))
                    || trimmed.starts_with(&format!("extern crate {rust_name}"))
                {
                    violations.push((
                        path.file_name().unwrap().to_string_lossy().to_string(),
                        line_num + 1,
                        format!("imports banned crate '{framework}'"),
                    ));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Source files import banned crates:\n{}",
        violations
            .iter()
            .map(|(file, line, msg)| format!("  {file}:{line}: {msg}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// -------------------------------------------------------------------------
// Dependency isolation: parse Cargo.toml and verify no web frameworks
// -------------------------------------------------------------------------

#[test]
fn cargo_toml_does_not_list_web_framework_dependencies() {
    let cargo_toml_path = project_root().join("Cargo.toml");
    let contents = fs::read_to_string(&cargo_toml_path)
        .expect("failed to read Cargo.toml");

    // Parse the TOML to inspect [dependencies] specifically
    // We do a simple section-aware parse to distinguish [dependencies]
    // from [dev-dependencies] (web frameworks in dev-dependencies are fine).
    let mut in_dependencies_section = false;
    let mut in_dev_dependencies_section = false;
    let mut violations: Vec<(usize, String)> = Vec::new();

    for (line_num, line) in contents.lines().enumerate() {
        let trimmed = line.trim();

        // Detect section headers
        if trimmed.starts_with('[') {
            let section = trimmed
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_lowercase();

            in_dependencies_section = section == "dependencies";
            in_dev_dependencies_section = section == "dev-dependencies";
            continue;
        }

        // Only check lines in [dependencies], not [dev-dependencies]
        if !in_dependencies_section || in_dev_dependencies_section {
            continue;
        }

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        for &framework in BANNED_FRAMEWORK_CRATES {
            // Check if the line declares this framework as a dependency
            // Handles both `actix-web = "..."` and `actix-web = { ... }`
            if trimmed.starts_with(framework)
                && trimmed[framework.len()..]
                    .trim_start()
                    .starts_with('=')
            {
                violations.push((
                    line_num + 1,
                    format!(
                        "Cargo.toml [dependencies] lists banned web framework '{framework}': {trimmed}"
                    ),
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Cargo.toml contains banned web framework dependencies:\n{}",
        violations
            .iter()
            .map(|(line, msg)| format!("  line {line}: {msg}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn cargo_toml_only_has_expected_dependencies() {
    let cargo_toml_path = project_root().join("Cargo.toml");
    let contents = fs::read_to_string(&cargo_toml_path)
        .expect("failed to read Cargo.toml");

    let mut in_dependencies_section = false;
    let mut dep_names: Vec<String> = Vec::new();

    for line in contents.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with('[') {
            let section = trimmed
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
                .to_lowercase();
            in_dependencies_section = section == "dependencies";
            continue;
        }

        if !in_dependencies_section {
            continue;
        }

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Extract the dependency name (everything before '=')
        if let Some(eq_pos) = trimmed.find('=') {
            let name = trimmed[..eq_pos].trim();
            dep_names.push(name.to_string());
        }
    }

    // Every dependency should be in the allowed list
    let unexpected: Vec<&String> = dep_names
        .iter()
        .filter(|name| !ALLOWED_CRATES.contains(&name.as_str()))
        .collect();

    // If there are unexpected deps, verify at least none are banned frameworks
    for dep in &unexpected {
        for &framework in BANNED_FRAMEWORK_CRATES {
            assert_ne!(
                dep.as_str(),
                framework,
                "Cargo.toml [dependencies] contains banned web framework: {framework}"
            );
        }
    }
}

#[test]
fn no_web_framework_in_cargo_toml_anywhere_as_required_dep() {
    // Belt-and-suspenders: also do a regex-free substring scan of the entire
    // Cargo.toml, but only flag if the framework name appears as a key in what
    // looks like a dependency line (not in comments, not in [dev-dependencies]).
    let cargo_toml_path = project_root().join("Cargo.toml");
    let contents = fs::read_to_string(&cargo_toml_path)
        .expect("failed to read Cargo.toml");

    // Extract just the [dependencies] section text
    let dep_section = extract_section(&contents, "dependencies");

    if let Some(section_text) = dep_section {
        for &framework in BANNED_FRAMEWORK_CRATES {
            // Check that the framework doesn't appear as a dependency key
            let pattern = format!("\n{framework}");
            assert!(
                !section_text.contains(&pattern)
                    && !section_text.starts_with(framework),
                "[dependencies] section contains banned framework '{framework}'"
            );
        }
    }
}

/// Extract the text content of a TOML section by name.
/// Returns None if the section doesn't exist.
fn extract_section(toml_content: &str, section_name: &str) -> Option<String> {
    let header = format!("[{section_name}]");
    let start = toml_content.find(&header)?;
    let after_header = start + header.len();
    let rest = &toml_content[after_header..];

    // Find the next section header
    let end = rest.find("\n[").map(|pos| pos).unwrap_or(rest.len());

    Some(rest[..end].to_string())
}
