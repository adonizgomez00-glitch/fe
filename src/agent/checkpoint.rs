use std::path::{Path, PathBuf};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub timestamp: String,
    pub session_id: String,
    pub context_usage_pct: f64,
    pub tokens_used: u32,
    pub max_context: u32,
    pub turns_completed: u32,
    pub files_modified: Vec<String>,
    pub summary: String,
    pub next_action: String,
}

pub struct CheckpointManager {
    checkpoints_dir: PathBuf,
}

impl CheckpointManager {
    pub fn new(checkpoints_dir: &Path) -> Self {
        Self {
            checkpoints_dir: checkpoints_dir.to_path_buf(),
        }
    }

    pub fn save(&self, cp: &Checkpoint) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.checkpoints_dir)?;

        let filename = format!("checkpoint_{}.md", cp.timestamp.replace([' ', ':'], "-"));
        let path = self.checkpoints_dir.join(&filename);
        let content = self.format_checkpoint(cp);
        std::fs::write(&path, content)?;

        self.cleanup_old()?;

        Ok(path)
    }

    pub fn save_session_checkpoint(&self, session_id: &str, cp: &Checkpoint) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.checkpoints_dir)?;

        let filename = format!("{}.checkpoint.md", session_id);
        let path = self.checkpoints_dir.join(&filename);
        let content = self.format_checkpoint(cp);
        std::fs::write(&path, content)?;

        Ok(path)
    }

    pub fn load_latest(&self, session_id: &str) -> Option<Checkpoint> {
        let path = self.checkpoints_dir.join(format!("{}.checkpoint.md", session_id));
        if path.exists() {
            std::fs::read_to_string(&path).ok().and_then(|c| self.parse_checkpoint(&c))
        } else {
            None
        }
    }

    pub fn list(&self) -> Result<Vec<PathBuf>> {
        if !self.checkpoints_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries: Vec<_> = std::fs::read_dir(&self.checkpoints_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "md"))
            .collect();

        entries.sort_by_key(|p| p.metadata().ok().and_then(|m| m.modified().ok()));
        entries.reverse();
        Ok(entries)
    }

    fn format_checkpoint(&self, cp: &Checkpoint) -> String {
        let mut out = String::new();
        out.push_str(&format!("# CHECKPOINT — {}\n\n", cp.timestamp));
        out.push_str(&format!("- **Sesión**: {}\n", cp.session_id));
        out.push_str(&format!("- **Contexto**: {:.1}% ({}/{})\n", cp.context_usage_pct, cp.tokens_used, cp.max_context));
        out.push_str(&format!("- **Turnos completados**: {}\n", cp.turns_completed));
        out.push_str(&format!("- **Siguiente acción**: {}\n\n", cp.next_action));
        out.push_str("---\n\n");
        out.push_str("## Resumen\n\n");
        out.push_str(&cp.summary);
        out.push_str("\n\n---\n\n");
        if !cp.files_modified.is_empty() {
            out.push_str("## Archivos Modificados\n\n");
            for f in &cp.files_modified {
                out.push_str(&format!("- `{}`\n", f));
            }
            out.push_str("\n---\n\n");
        }
        out.push_str("## Instrucción de Reanudación\n\n");
        out.push_str("REANUDANDO desde checkpoint. Ignorar mensajes anteriores salvo este checkpoint.\n");
        out
    }

    fn parse_checkpoint(&self, content: &str) -> Option<Checkpoint> {
        let timestamp = content.lines()
            .find(|l| l.starts_with("# CHECKPOINT"))
            .and_then(|l| l.split("— ").nth(1))
            .unwrap_or("")
            .to_string();

        let session_id = content.lines()
            .find(|l| l.contains("**Sesión**"))
            .and_then(|l| l.split("**: ").nth(1))
            .unwrap_or("")
            .to_string();

        let next_action = content.lines()
            .find(|l| l.contains("**Siguiente acción**"))
            .and_then(|l| l.split("**: ").nth(1))
            .unwrap_or("")
            .to_string();

        let summary = content.split("## Resumen\n\n")
            .nth(1)
            .and_then(|s| s.split("\n\n---")
                .next())
            .unwrap_or("")
            .to_string();

        Some(Checkpoint {
            timestamp,
            session_id,
            context_usage_pct: 0.0,
            tokens_used: 0,
            max_context: 0,
            turns_completed: 0,
            files_modified: Vec::new(),
            summary,
            next_action,
        })
    }

    fn cleanup_old(&self) -> Result<()> {
        let entries = self.list()?;
        if entries.len() > 100 {
            for old in entries.iter().skip(100) {
                let _ = std::fs::remove_file(old);
            }
        }
        Ok(())
    }

    pub fn format_resume_prompt(&self, cp: &Checkpoint) -> String {
        format!(
            "## REANUDACIÓN DESDE CHECKPOINT ({timestamp})

Se ha alcanzado el 80% de la ventana de contexto.
El historial anterior ha sido resumido en el checkpoint.

### Contexto preservado:
- Sesión: {session_id}
- Contexto usado: {pct:.1}% ({used}/{max} tokens)
- Turnos completados: {turns}

### Estado actual:
{summary}

### Próxima acción:
{next_action}

INSTRUCCIÓN: Continúa la tarea desde el punto exacto donde se quedó.
NO repitas acciones ya completadas.
NO intentes re-leer el proyecto completo a menos que sea necesario.
Usa herramientas de lectura para acceder a archivos específicos si necesitas más contexto.",
            timestamp = cp.timestamp,
            session_id = cp.session_id,
            pct = cp.context_usage_pct,
            used = cp.tokens_used,
            max = cp.max_context,
            turns = cp.turns_completed,
            summary = cp.summary,
            next_action = cp.next_action,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_checkpoint() -> Checkpoint {
        Checkpoint {
            timestamp: "2026-07-17 12:00:00".into(),
            session_id: "test-session-123".into(),
            context_usage_pct: 82.5,
            tokens_used: 26400,
            max_context: 32000,
            turns_completed: 12,
            files_modified: vec!["src/agent/core.rs".into(), "src/config.rs".into()],
            summary: "Se implementó el checkpoint system en core.rs. El sistema ahora puede detectar cuando el contexto supera el 80% y ejecuta el protocolo de checkpoint: pausa, re-lectura del proyecto, documentación y reanudación.".into(),
            next_action: "Probar el checkpoint con una sesión larga".into(),
        }
    }

    #[test]
    fn test_checkpoint_save_and_load() {
        let dir = tempfile::tempdir().unwrap();
        let manager = CheckpointManager::new(dir.path());
        let cp = make_checkpoint();

        let path = manager.save_session_checkpoint("test-session-123", &cp).unwrap();
        assert!(path.exists());

        let loaded = manager.load_latest("test-session-123");
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().session_id, "test-session-123");
    }

    #[test]
    fn test_checkpoint_session_file() {
        let dir = tempfile::tempdir().unwrap();
        let manager = CheckpointManager::new(dir.path());
        let cp = make_checkpoint();

        let path = manager.save_session_checkpoint("session-abc", &cp).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("session-abc.checkpoint"));
    }

    #[test]
    fn test_checkpoint_save_timestamp_file() {
        let dir = tempfile::tempdir().unwrap();
        let manager = CheckpointManager::new(dir.path());
        let cp = make_checkpoint();

        let path = manager.save(&cp).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("checkpoint_"));
    }

    #[test]
    fn test_list_empty() {
        let dir = tempfile::tempdir().unwrap();
        let manager = CheckpointManager::new(dir.path());
        let list = manager.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_non_empty() {
        let dir = tempfile::tempdir().unwrap();
        let manager = CheckpointManager::new(dir.path());
        manager.save(&make_checkpoint()).unwrap();
        manager.save_session_checkpoint("other-session", &make_checkpoint()).unwrap();

        let list = manager.list().unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_format_resume_prompt() {
        let dir = tempfile::tempdir().unwrap();
        let manager = CheckpointManager::new(dir.path());
        let cp = make_checkpoint();

        let prompt = manager.format_resume_prompt(&cp);
        assert!(prompt.contains("REANUDACIÓN DESDE CHECKPOINT"));
        assert!(prompt.contains("test-session-123"));
        assert!(prompt.contains("82.5%"));
        assert!(prompt.contains("Se implementó el checkpoint system"));
        assert!(prompt.contains("Probar el checkpoint"));
    }

    #[test]
    fn test_load_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let manager = CheckpointManager::new(dir.path());
        let loaded = manager.load_latest("nonexistent");
        assert!(loaded.is_none());
    }

    #[test]
    fn test_checkpoint_fields() {
        let cp = make_checkpoint();
        assert_eq!(cp.turns_completed, 12);
        assert_eq!(cp.files_modified.len(), 2);
        assert!((cp.context_usage_pct - 82.5).abs() < 0.01);
    }
}
