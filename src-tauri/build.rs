use std::path::Path;

/// Caminho do `.env` relativo a `src-tauri/` (cwd dos build scripts).
const DOTENV_PATH: &str = "../.env";

/// Prefixo das variáveis que podem ser embutidas no binário.
const ENV_PREFIX: &str = "RETROSYNC_";

fn main() {
    load_dotenv();
    tauri_build::build()
}

/// Lê o `.env` da raiz do repositório e reexporta as variáveis `RETROSYNC_*`
/// via `cargo:rustc-env`, tornando-as visíveis ao `option_env!` do código.
/// Variáveis já definidas no ambiente do shell têm precedência sobre o arquivo.
fn load_dotenv() {
    println!("cargo:rerun-if-changed={DOTENV_PATH}");

    let Ok(content) = std::fs::read_to_string(Path::new(DOTENV_PATH)) else {
        return;
    };

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if !key.starts_with(ENV_PREFIX) {
            continue;
        }
        println!("cargo:rerun-if-env-changed={key}");
        if std::env::var(key).is_ok() {
            continue;
        }
        let value = value.trim().trim_matches('"').trim_matches('\'');
        if value.is_empty() {
            continue;
        }
        println!("cargo:rustc-env={key}={value}");
    }
}
