use std::fs;
use std::path::Path;

#[test]
fn test_gitignore_protection_env() {
    let gitignore =
        fs::read_to_string(".gitignore").expect(".gitignore file not found in project root");

    // Check that .env and .pem files are blocked
    let required_patterns = vec![
        ".env",
        "*.env",
        "*.env.local",
        ".env.production",
        ".env.secret",
        "*.pem",
        "!public_key.pem",
        "*.key",
        "private_key.pem",
        "privkey.pem",
        "ec.key",
        "chiave_privata.key",
    ];

    for pattern in required_patterns {
        let has_exact = gitignore.contains(&pattern);
        let has_wildcard = gitignore.contains(&format!("*.env.local"))
            || gitignore.contains(&format!(".env.local"));
        let has_env = gitignore.contains("*.env") || gitignore.contains(".env");

        // For .env.local, either .env.local or *.env.local is acceptable
        let is_env_local = pattern == "*.env.local" || pattern == ".env.local";
        if is_env_local {
            assert!(
                has_wildcard,
                ".gitignore must contain pattern '*.env.local' or '.env.local' to protect secrets",
            );
        } else if pattern == ".env.production" || pattern == ".env.secret" {
            assert!(
                gitignore.contains(pattern),
                ".gitignore must contain pattern '{}' to protect secrets",
                pattern
            );
        } else if pattern == "*.env" {
            assert!(
                has_env,
                ".gitignore must contain pattern '*.env' or '.env' to protect secrets",
            );
        } else if pattern == ".env" {
            assert!(
                has_env,
                ".gitignore must contain pattern '*.env' or '.env' to protect secrets",
            );
        } else {
            assert!(
                gitignore.contains(pattern),
                ".gitignore must contain pattern '{}' to protect secrets",
                pattern
            );
        }
    }

    println!(".gitignore properly protects .env, .pem, and .key files");
}

#[test]
fn test_no_private_key_in_git() {
    // Check that .gitignore includes private_key.pem
    let gitignore = match fs::read_to_string(".gitignore") {
        Ok(c) => c,
        Err(e) => {
            println!("WARNING: .gitignore not found: {}", e);
            return;
        }
    };

    assert!(
        gitignore.contains("private_key.pem"),
        ".gitignore must block private_key.pem"
    );
    assert!(
        gitignore.contains("privkey.pem"),
        ".gitignore must block privkey.pem"
    );
    assert!(gitignore.contains("ec.key"), ".gitignore must block ec.key");
    assert!(
        gitignore.contains("chiave_privata.key"),
        ".gitignore must block chiave_privata.key"
    );

    // Check that no private key files are tracked by git
    let output = std::process::Command::new("git")
        .args(&["ls-files", "*.pem", "*.key"])
        .output()
        .expect("Failed to run git ls-files");

    let tracked_keys = String::from_utf8(output.stdout).unwrap();
    let tracked_keys: Vec<&str> = tracked_keys.lines().collect();

    // Only non-empty entries and only public_key.pem should be tracked
    for tracked in tracked_keys.iter().filter(|s| !s.is_empty()) {
        if !tracked.contains("public_key.pem") {
            assert!(
                false,
                "Private key file is tracked by git: {}. Remove it with git rm --cached",
                tracked
            );
        }
    }

    println!("PASS: No private keys tracked in git (only public_key.pem allowed)");
}

#[test]
fn test_no_token_in_source_files() {
    // Scan source files for hardcoded tokens
    let mut found_issues = Vec::new();

    // Scan .sh files for hardcoded 40-char hex strings
    for entry in fs::read_dir(".").unwrap().filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(ext) = path.extension() {
            if ext == "sh" {
                let content = fs::read_to_string(&path).unwrap();
                for (line_num, line) in content.lines().enumerate() {
                    // Skip comments and example/template files
                    if line.trim().starts_with("#")
                        || line.to_lowercase().contains("example")
                        || line.to_lowercase().contains("template")
                    {
                        continue;
                    }
                    // Check for 40-64 hex chars that could be API tokens (not in .env.example comments)
                    if line.trim().len() >= 40 {
                        let hex_chars = line
                            .trim()
                            .chars()
                            .filter(|c| c.is_ascii_hexdigit())
                            .collect::<Vec<_>>();
                        if hex_chars.len() >= 40 && hex_chars.len() <= 64 {
                            // Check if it looks like it's part of a TOKEN assignment
                            if line.to_lowercase().contains("token")
                                || line.to_lowercase().contains("api")
                                || line.to_lowercase().contains("secret")
                            {
                                found_issues.push(format!(
                                    "Potential hardcoded token in {}: line {}: {}",
                                    path.display(),
                                    line_num + 1,
                                    line.trim()
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if !found_issues.is_empty() {
        println!("FAIL: Found potential hardcoded tokens:");
        for issue in &found_issues {
            println!("  {}", issue);
        }
        assert!(
            false,
            "Found potential hardcoded tokens in shell scripts: {:?}",
            found_issues
        );
    }

    println!("PASS: No hardcoded tokens found in shell scripts");
}
