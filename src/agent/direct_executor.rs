use anyhow::Result;
use regex::Regex;
use std::sync::LazyLock;
use std::os::unix::fs::PermissionsExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tokio::fs;

static RE_LIST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|\s)(?:list[ae]|muestra|enseña|haz\s*(?:me\s*)?(?:un\s*)?ls)\s*(?:los\s+)?(?:archivos|directorios|elementos|contenido)?|^\s*ls\b").unwrap()
});

static RE_PWD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:donde\s+(?:estoy|me\s+encuentro|estamos|está)|(?:en\s+)?(?:qué|que)\s+directorio\s+(?:estoy|es\s+este|me\s+encuentro)|(?:en\s+)?que\s+(?:carpeta|ruta)\s+(?:estoy|es\s+esta)|directorio\s+actual|ruta\s+actual|carpeta\s+actual|pwd)$").unwrap()
});

static RE_TOUCH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:crea|haz\s*(?:me\s*)?(?:un\s*)?)\s*(?:un\s+)?archivo\s+(?:vac[íi]o\s+)?(?:llamado\s+)?(\S+)\s*$").unwrap()
});

static RE_WRITE_FILE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*crea\s*(?:un\s+)?archivo\s*(?:llamado\s+)?(\S+)\s+y\s+escribe\s+(.+)$").unwrap()
});

static RE_UPDATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:^|\s)actualiza\s*(?:los\s+)?(?:repositorios|sistema|paquetes)").unwrap()
});

static RE_MKDIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:crea|haz\s*(?:me\s*)?(?:un\s*)?)\s*(?:un\s+)?directorio\s*(?:llamado\s+)?(\S+)\s*$").unwrap()
});

static RE_RMDIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:elimina|borra|quita|haz\s*(?:me\s*)?)\s*(?:el\s+)?directorio\s*(?:llamado\s+)?(\S+)\s*$").unwrap()
});

static RE_RM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:elimina|borra|quita|haz\s*(?:me\s*)?)\s*(?:el\s+)?archivo\s*(?:llamado\s+)?(\S+)\s*$").unwrap()
});

static RE_CAT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:muestra|enseña|cat|lee|haz\s*(?:me\s+)?cat)\s*(?:el\s+)?(?:contenido\s+(?:de\s+)?)?(?:archivo\s+)?(\S+)\s*$").unwrap()
});

static RE_SEARCH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*busca\s*(?:el\s+)?(?:archivo\s+)?(\S+)$").unwrap()
});

static RE_CREATE_APP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:crea|desarrolla|genera|haz\s*(?:me\s*)?(?:una\s*)?)\s*(?:una\s+|un\s+)?(?:app|aplicaci[óo]n|script|programa|archivo)\s+(python|py|node|js|rust|rs|bash|sh)(?:\s+(?:cli|desktop|web|admin|doc|terminal|gui))?(?:\s+(?:llamado|llamada)\s+(\S+))?\s+que\s+(.+)$").unwrap()
});

pub struct DirectMatch {
    pub description: String,
    pub command: String,
    pub is_destructive: bool,
}

pub fn match_prompt(prompt: &str) -> Option<DirectMatch> {
    let prompt = prompt.trim();

    if prompt.is_empty() {
        return None;
    }

    if RE_PWD.is_match(prompt) {
        return Some(DirectMatch {
            description: "Mostrar directorio actual".into(),
            command: "pwd".into(),
            is_destructive: false,
        });
    }

    if RE_UPDATE.is_match(prompt) {
        return Some(DirectMatch {
            description: "Actualizar repositorios".into(),
            command: "apt update 2>&1".into(),
            is_destructive: false,
        });
    }

    if RE_CAT.is_match(prompt) {
        let caps = RE_CAT.captures(prompt).unwrap();
        let file_name = caps.get(1).unwrap().as_str().trim();
        if !file_name.is_empty() {
            return Some(DirectMatch {
                description: format!("Mostrar contenido de '{}'", file_name),
                command: format!("cat {}", sh_quote(file_name)),
                is_destructive: false,
            });
        }
    }

    if RE_LIST.is_match(prompt) {
        return Some(DirectMatch {
            description: "Listar archivos y directorios".into(),
            command: "ls -la".into(),
            is_destructive: false,
        });
    }

    if let Some(caps) = RE_MKDIR.captures(prompt) {
        let dir_name = caps.get(1).unwrap().as_str().trim();
        if !dir_name.is_empty() {
            return Some(DirectMatch {
                description: format!("Crear directorio '{}'", dir_name),
                command: format!("mkdir -p {}", sh_quote(dir_name)),
                is_destructive: false,
            });
        }
    }

    if let Some(caps) = RE_RMDIR.captures(prompt) {
        let dir_name = caps.get(1).unwrap().as_str().trim();
        if !dir_name.is_empty() {
            return Some(DirectMatch {
                description: format!("Eliminar directorio '{}'", dir_name),
                command: format!("rm -rf {}", sh_quote(dir_name)),
                is_destructive: true,
            });
        }
    }

    if let Some(caps) = RE_RM.captures(prompt) {
        let file_name = caps.get(1).unwrap().as_str().trim();
        if !file_name.is_empty() {
            return Some(DirectMatch {
                description: format!("Eliminar archivo '{}'", file_name),
                command: format!("rm {}", sh_quote(file_name)),
                is_destructive: true,
            });
        }
    }

    if let Some(caps) = RE_SEARCH.captures(prompt) {
        let pattern = caps.get(1).unwrap().as_str().trim();
        if !pattern.is_empty() {
            return Some(DirectMatch {
                description: format!("Buscar '{}'", pattern),
                command: format!("find . -name '*{}*' 2>/dev/null", sh_quote(pattern)),
                is_destructive: false,
            });
        }
    }

    if let Some(caps) = RE_TOUCH.captures(prompt) {
        let file_name = caps.get(1).unwrap().as_str().trim();
        if !file_name.is_empty() {
            return Some(DirectMatch {
                description: format!("Crear archivo '{}'", file_name),
                command: format!("touch {}", sh_quote(file_name)),
                is_destructive: false,
            });
        }
    }

    if let Some(caps) = RE_WRITE_FILE.captures(prompt) {
        let file_name = caps.get(1).unwrap().as_str().trim();
        let content = caps.get(2).unwrap().as_str().trim();
        if !file_name.is_empty() {
            let quoted_name = sh_quote(file_name);
            let quoted_content = sh_quote(content);
            return Some(DirectMatch {
                description: format!("Crear archivo '{}' con contenido", file_name),
                command: format!("printf '%s' {} > {}", quoted_content, quoted_name),
                is_destructive: false,
            });
        }
    }

    if let Some(caps) = RE_CREATE_APP.captures(prompt) {
        let lang = caps.get(1).unwrap().as_str().to_lowercase();
        let app_name = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
        let description = caps.get(3).unwrap().as_str().trim();

        let lang_normalized = crate::agent::templates::normalize_language(&lang);
        let app_type = crate::agent::templates::detect_app_type(description);

        let template = crate::agent::templates::generate_app(
            lang_normalized, app_type, app_name, description,
        );

        let commands = crate::agent::templates::project_to_commands(&template);
        let (desc, cmd) = &commands[0];

        return Some(DirectMatch {
            description: desc.clone(),
            command: cmd.clone(),
            is_destructive: false,
        });
    }

    None
}

pub fn sh_quote(s: &str) -> String {
    if s.contains(char::is_whitespace) || s.contains(['\'', '"', '$', '`', '\\']) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

pub async fn execute_direct(dm: &DirectMatch) -> Result<String> {
    let cmd = &dm.command;
    
    // If command contains newlines (multi-line script), write to temp file and execute
    let output = if cmd.contains('\n') {
        let temp_file = format!("/tmp/fe_direct_{}.sh", uuid::Uuid::new_v4().simple());
        fs::write(&temp_file, format!("#!/bin/bash\nset -euo pipefail\n{}", cmd)).await?;
        fs::set_permissions(&temp_file, std::fs::Permissions::from_mode(0o755)).await?;
        
        timeout(
            Duration::from_secs(30),
            Command::new("bash").arg(&temp_file).output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Timeout ejecutando script: {}", temp_file))?
        .map_err(|e| anyhow::anyhow!("Error ejecutando script '{}': {}", temp_file, e))?
    } else {
        timeout(
            Duration::from_secs(30),
            Command::new("sh").arg("-c").arg(cmd).output(),
        )
        .await
        .map_err(|_| anyhow::anyhow!("Timeout ejecutando: {}", cmd))?
        .map_err(|e| anyhow::anyhow!("Error ejecutando '{}': {}", cmd, e))?
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code();

    let mut result = stdout;
    if !stderr.is_empty() {
        result.push_str("\n--- stderr ---\n");
        result.push_str(&stderr);
    }
    if exit_code != Some(0) {
        result.push_str(&format!("\n(exit code: {:?})", exit_code));
    }

    Ok(result)
}

pub fn format_output(dm: &DirectMatch, output: &str) -> String {
    let icon = if dm.is_destructive { "🔴" } else { "✅" };
    if output.trim().is_empty() {
        format!("{} {} — Completado", icon, dm.description)
    } else {
        format!("{} {}\n{}", icon, dm.description, output.trim())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_list() {
        let r = match_prompt("lista los archivos");
        assert!(r.is_some());
        assert!(!r.unwrap().is_destructive);
    }

    #[test]
    fn test_match_list_variant() {
        let r = match_prompt("lista archivos y directorios");
        assert!(r.is_some());
    }

    #[test]
    fn test_match_list_muestra() {
        let r = match_prompt("muestra los archivos");
        assert!(r.is_some());
    }

    #[test]
    fn test_match_pwd() {
        for prompt in &[
            "donde estoy",
            "directorio actual",
            "pwd",
            "en que directorio estoy",
            "en qué directorio estoy",
            "que directorio es este",
            "donde me encuentro",
            "en que carpeta estoy",
            "ruta actual",
        ] {
            let r = match_prompt(prompt);
            assert!(r.is_some(), "debería matchear: {}", prompt);
        }
    }

    #[test]
    fn test_match_mkdir() {
        let r = match_prompt("crea un directorio llamado prueba");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("mkdir"));
        assert!(!dm.is_destructive);
    }

    #[test]
    fn test_match_mkdir_short() {
        let r = match_prompt("crea directorio prueba");
        assert!(r.is_some());
    }

    #[test]
    fn test_match_rmdir() {
        let r = match_prompt("elimina el directorio prueba");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("rm -rf"));
        assert!(dm.is_destructive);
    }

    #[test]
    fn test_match_rm() {
        let r = match_prompt("elimina el archivo hola.py");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("rm "));
        assert!(dm.is_destructive);
    }

    #[test]
    fn test_match_cat() {
        let r = match_prompt("muestra el contenido de README.md");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("cat"));
    }

    #[test]
    fn test_match_update() {
        let r = match_prompt("actualiza repositorios");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("apt update"));
    }

    #[test]
    fn test_match_search() {
        let r = match_prompt("busca main.rs");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("find"));
    }

    #[test]
    fn test_no_match_complex() {
        let r = match_prompt("crea un directorio y elimina otro");
        assert!(r.is_none());
    }

    #[test]
    fn test_no_match_empty() {
        let r = match_prompt("");
        assert!(r.is_none());
    }

    #[test]
    fn test_match_list_does_not_match_crea() {
        let r = match_prompt("crea un archivo");
        assert!(r.is_none());
    }

    #[tokio::test]
    async fn test_execute_pwd() {
        let dm = DirectMatch {
            description: "test".into(),
            command: "echo hola".into(),
            is_destructive: false,
        };
        let out = execute_direct(&dm).await.unwrap();
        assert!(out.contains("hola"));
    }

    #[test]
    fn test_sh_quote_simple() {
        assert_eq!(sh_quote("foo"), "foo");
    }

    #[test]
    fn test_sh_quote_with_spaces() {
        let q = sh_quote("mi directorio");
        assert_eq!(q, "'mi directorio'");
    }

    #[test]
    fn test_format_output_empty() {
        let dm = DirectMatch {
            description: "Test".into(),
            command: "".into(),
            is_destructive: false,
        };
        let out = format_output(&dm, "");
        assert!(out.contains("Completado"));
    }

    #[test]
    fn test_match_create_app_python() {
        for prompt in &[
            "crea una app python que sume o reste",
            "crea un script python llamado calculadora que sume y reste",
            "crea una aplicacion python que reste",
            "desarrolla una app python que sume",
            "haz una app python que sume y reste",
            "genera una app python que sume",
            "crea un archivo python que sume",
        ] {
            let r = match_prompt(prompt);
            assert!(r.is_some(), "deberia matchear: {}", prompt);
            let dm = r.unwrap();
            assert!(!dm.is_destructive);
            assert!(dm.command.contains("mkdir -p"));
            assert!(dm.command.contains("cat >"));
        }
    }

    #[test]
    fn test_match_create_app_node() {
        let r = match_prompt("crea una app node que sume");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("app_node"));
        assert!(dm.command.contains(".js"));
    }

    #[test]
    fn test_match_create_app_bash() {
        let r = match_prompt("crea un script bash que reste");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("app_bash"));
        assert!(dm.command.contains(".sh"));
    }

    #[test]
    fn test_match_create_app_rust() {
        let r = match_prompt("crea una app rust que sume y reste");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("app_rust"));
        assert!(dm.command.contains(".rs"));
    }

    #[test]
    fn test_match_create_app_with_custom_name() {
        let r = match_prompt("crea un script python llamado mi_app que sume");
        assert!(r.is_some());
        let dm = r.unwrap();
        assert!(dm.command.contains("mi_app"));
        assert!(dm.command.contains("mi_app.py"));
    }

}
