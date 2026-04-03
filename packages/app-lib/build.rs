use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, exit};
use std::{env, fs};

fn main() {
    println!("cargo::rerun-if-changed=.env");
    println!("cargo::rerun-if-changed=java/gradle");
    println!("cargo::rerun-if-changed=java/src");
    println!("cargo::rerun-if-changed=java/build.gradle.kts");
    println!("cargo::rerun-if-changed=java/settings.gradle.kts");
    println!("cargo::rerun-if-changed=java/gradle.properties");

    set_env();
    build_gdk_secrets();
    build_java_jars();
}

fn set_env() {
    for (var_name, var_value) in
        dotenvy::dotenv_iter().into_iter().flatten().flatten()
    {
        if var_name == "DATABASE_URL" {
            // The sqlx database URL is a build-time detail that should not be exposed to the crate
            continue;
        }

        println!("cargo::rustc-env={var_name}={var_value}");
    }
}

fn build_java_jars() {
    let out_dir =
        dunce::canonicalize(PathBuf::from(env::var_os("OUT_DIR").unwrap()))
            .unwrap();

    println!(
        "cargo::rustc-env=JAVA_JARS_DIR={}",
        out_dir.join("java/libs").display()
    );

    let gradle_path = fs::canonicalize(
        #[cfg(target_os = "windows")]
        "java\\gradlew.bat",
        #[cfg(not(target_os = "windows"))]
        "java/gradlew",
    )
    .unwrap();

    let mut build_dir_str = OsString::from("-Dorg.gradle.project.buildDir=");
    build_dir_str.push(out_dir.join("java"));
    let exit_status = Command::new(gradle_path)
        .arg(build_dir_str)
        .arg("build")
        .arg("--no-daemon")
        .arg("--console=rich")
        .current_dir(dunce::canonicalize("java").unwrap())
        .status()
        .expect("Failed to wait on Gradle build");

    if !exit_status.success() {
        println!("cargo::error=Gradle build failed with {exit_status}");
        exit(exit_status.code().unwrap_or(1));
    }
}

fn build_gdk_secrets() {
    println!("cargo::rerun-if-env-changed=GDK_RELEASE_KEY");
    println!("cargo::rerun-if-env-changed=GDK_PREVIEW_KEY");
    println!("cargo::rerun-if-env-changed=GDK_RELEASE_KEY_FILE");
    println!("cargo::rerun-if-env-changed=GDK_PREVIEW_KEY_FILE");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let secrets_path = out_dir.join("secrets.rs");

    let release = resolve_gdk_key_hex(
        "GDK_RELEASE_KEY",
        "GDK_RELEASE_KEY_FILE",
        &[
            "src/launcher/gdk/Cik/bdb9e791-c97c-3734-e1a8-bc602552df06.cik",
            "src/core/minecraft/gdk/Cik/bdb9e791-c97c-3734-e1a8-bc602552df06.cik",
            "../Better-Minecraft-Bedrock-Launcher/src-tauri/src/core/minecraft/gdk/Cik/bdb9e791-c97c-3734-e1a8-bc602552df06.cik",
            "../gdk-offline-install-feature/backend/Cik/bdb9e791-c97c-3734-e1a8-bc602552df06.cik",
        ],
    );
    let preview = resolve_gdk_key_hex(
        "GDK_PREVIEW_KEY",
        "GDK_PREVIEW_KEY_FILE",
        &[
            "src/launcher/gdk/Cik/1f49d63f-8bf5-1f8d-ed7e-dbd89477dad9.cik",
            "src/core/minecraft/gdk/Cik/1f49d63f-8bf5-1f8d-ed7e-dbd89477dad9.cik",
            "../Better-Minecraft-Bedrock-Launcher/src-tauri/src/core/minecraft/gdk/Cik/1f49d63f-8bf5-1f8d-ed7e-dbd89477dad9.cik",
            "../gdk-offline-install-feature/backend/Cik/1f49d63f-8bf5-1f8d-ed7e-dbd89477dad9.cik",
        ],
    );

    let body = format!(
        "pub const RELEASE_KEY_HEX: Option<&str> = {};\npub const PREVIEW_KEY_HEX: Option<&str> = {};\n",
        release
            .as_deref()
            .map(|v| format!("Some(\"{}\")", escape_rust_string(v)))
            .unwrap_or_else(|| "None".to_string()),
        preview
            .as_deref()
            .map(|v| format!("Some(\"{}\")", escape_rust_string(v)))
            .unwrap_or_else(|| "None".to_string())
    );

    fs::write(secrets_path, body).expect("failed to write gdk secrets.rs");
}

fn resolve_gdk_key_hex(
    env_hex_name: &str,
    env_file_name: &str,
    candidate_paths: &[&str],
) -> Option<String> {
    if let Ok(hex_val) = env::var(env_hex_name) {
        let clean: String = hex_val
            .chars()
            .filter(|c| c.is_ascii_hexdigit())
            .collect();
        if clean.len() >= 96 {
            return Some(clean);
        }
    }

    if let Ok(path) = env::var(env_file_name)
        && let Some(hex) = read_cik_hex(PathBuf::from(path))
    {
        return Some(hex);
    }

    for rel in candidate_paths {
        let path = PathBuf::from(rel);
        println!("cargo::rerun-if-changed={}", path.display());
        if let Some(hex) = read_cik_hex(path) {
            return Some(hex);
        }
    }

    None
}

fn read_cik_hex(path: PathBuf) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    Some(hex::encode(bytes))
}

fn escape_rust_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}
