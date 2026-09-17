use std::path::{Path, PathBuf};
use anyhow::Result;
use chrono::Local;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub start_time: String,
    pub model: String,
    pub skills_active: Vec<String>,
    pub turns: Vec<Turn>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Turn {
    pub timestamp: String,
    pub prompt: String,
    pub assistant_response: String,
}

pub struct SessionManager {
    sessions_dir: PathBuf,
    current_session: Option<Session>,
}

impl SessionManager {
    pub fn new(sessions_dir: &Path) -> Self {
        Self {
            sessions_dir: sessions_dir.to_path_buf(),
            current_session: None,
        }
    }

    pub fn create(&mut self, model: &str, skills_active: &[String]) -> Result<&Session> {
        std::fs::create_dir_all(&self.sessions_dir)?;

        let id = uuid::Uuid::new_v4().to_string();
        let session = Session {
            id,
            start_time: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            model: model.to_string(),
            skills_active: skills_active.to_vec(),
            turns: Vec::new(),
            summary: None,
        };

        self.current_session = Some(session);
        Ok(self.current_session.as_ref().unwrap())
    }

    pub fn current(&self) -> Option<&Session> {
        self.current_session.as_ref()
    }

    pub fn current_mut(&mut self) -> Option<&mut Session> {
        self.current_session.as_mut()
    }

    pub fn add_turn(&mut self, prompt: &str, response: &str) -> Result<()> {
        if let Some(ref mut session) = self.current_session {
            let turn = Turn {
                timestamp: Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                prompt: prompt.to_string(),
                assistant_response: response.to_string(),
            };
            session.turns.push(turn);
            self.save_current()?;
        }
        Ok(())
    }

    pub fn save_current(&self) -> Result<()> {
        if let Some(ref session) = self.current_session {
            let path = self.sessions_dir.join(format!("{}.md", session.id));
            let content = self.format_session(session);
            std::fs::write(&path, content)?;
        }
        Ok(())
    }

    fn format_session(&self, session: &Session) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Sesión {}\n\n", session.id));
        out.push_str(&format!("- **Inicio**: {}\n", session.start_time));
        out.push_str(&format!("- **Modelo**: {}\n", session.model));

        if !session.skills_active.is_empty() {
            out.push_str(&format!("- **Skills**: {}\n", session.skills_active.join(", ")));
        }

        out.push_str("\n---\n\n");

        if let Some(ref summary) = session.summary {
            out.push_str(&format!("## Resumen de Sesión\n\n{}\n\n---\n\n", summary));
        }

        for (i, turn) in session.turns.iter().enumerate() {
            out.push_str(&format!("## Turno {}\n\n", i + 1));
            out.push_str(&format!("**Hora**: {}\n\n", turn.timestamp));
            out.push_str(&format!("### Prompt\n\n{}\n\n", turn.prompt));
            out.push_str(&format!("### Respuesta\n\n{}\n\n", turn.assistant_response));
            out.push_str("---\n\n");
        }

        out
    }

    pub fn list(&self) -> Result<Vec<SessionSummary>> {
        let mut summaries = Vec::new();

        if !self.sessions_dir.exists() {
            return Ok(summaries);
        }

        let mut entries: Vec<_> = std::fs::read_dir(&self.sessions_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
            .collect();

        entries.sort_by_key(|e| e.path().metadata().ok().and_then(|m| m.modified().ok()));
        entries.reverse();

        for entry in entries {
            let content = std::fs::read_to_string(entry.path())?;
            if let Some(summary) = Self::parse_session_summary(&entry.path(), &content) {
                summaries.push(summary);
            }
        }

        Ok(summaries)
    }

    fn parse_session_summary(path: &Path, content: &str) -> Option<SessionSummary> {
        let id = path.file_stem()?.to_str()?.to_string();

        let start_time = content
            .lines()
            .find(|l| l.starts_with("- **Inicio**:"))
            .and_then(|l| l.split("**: ").nth(1))
            .unwrap_or("")
            .to_string();

        let model = content
            .lines()
            .find(|l| l.starts_with("- **Modelo**:"))
            .and_then(|l| l.split("**: ").nth(1))
            .unwrap_or("")
            .to_string();

        let skills = content
            .lines()
            .find(|l| l.starts_with("- **Skills**:"))
            .and_then(|l| l.split("**: ").nth(1))
            .unwrap_or("")
            .to_string();

        let turn_count = content.matches("## Turno ").count();
        let preview = content
            .lines()
            .skip_while(|l| !l.starts_with("### Prompt"))
            .nth(1)
            .unwrap_or("")
            .trim()
            .chars()
            .take(120)
            .collect();

        Some(SessionSummary {
            id,
            start_time,
            model,
            skills,
            turn_count,
            preview,
        })
    }

    pub fn show(&self, id: &str) -> Result<Option<String>> {
        let path = self.sessions_dir.join(format!("{}.md", id));
        if path.exists() {
            Ok(Some(std::fs::read_to_string(path)?))
        } else {
            Ok(None)
        }
    }

    pub fn export(&self, id: &str, output: &Path) -> Result<bool> {
        let path = self.sessions_dir.join(format!("{}.md", id));
        if !path.exists() {
            return Ok(false);
        }
        std::fs::copy(&path, output)?;
        Ok(true)
    }

    pub fn set_summary(&mut self, summary: &str) -> Result<()> {
        if let Some(ref mut session) = self.current_session {
            session.summary = Some(summary.to_string());
            self.save_current()?;
        }
        Ok(())
    }

    pub fn get_summary(&self) -> Option<&str> {
        self.current_session
            .as_ref()
            .and_then(|s| s.summary.as_deref())
    }

    pub fn clean(&self, older_than_days: u64) -> Result<usize> {
        let mut removed = 0;
        if !self.sessions_dir.exists() {
            return Ok(0);
        }

        let now = std::time::SystemTime::now();
        for entry in std::fs::read_dir(&self.sessions_dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|ext| ext == "md") {
                if let Ok(metadata) = entry.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        let age = now.duration_since(modified).unwrap_or_default();
                        if age.as_secs() > older_than_days * 86400 {
                            std::fs::remove_file(entry.path())?;
                            removed += 1;
                        }
                    }
                }
            }
        }
        Ok(removed)
    }
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub start_time: String,
    pub model: String,
    pub skills: String,
    pub turn_count: usize,
    pub preview: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn make_manager() -> (SessionManager, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(dir.path());
        (manager, dir)
    }

    #[test]
    fn test_create_session() {
        let (mut manager, _dir) = make_manager();
        let session = manager.create("qwen2.5:3b", &[]).unwrap();
        assert_eq!(session.model, "qwen2.5:3b");
        assert!(!session.id.is_empty());
        assert!(session.turns.is_empty());
    }

    #[test]
    fn test_create_session_with_skills() {
        let (mut manager, _dir) = make_manager();
        let skills = vec!["c-sysadmin".to_string(), "a-testing".to_string()];
        let session = manager.create("qwen2.5:3b", &skills).unwrap();
        assert_eq!(session.skills_active.len(), 2);
        assert!(session.skills_active.contains(&"c-sysadmin".to_string()));
    }

    #[test]
    fn test_add_turn() {
        let (mut manager, _dir) = make_manager();
        manager.create("test-model", &[]).unwrap();
        manager.add_turn("prompt test", "response test").unwrap();

        let session = manager.current().unwrap();
        assert_eq!(session.turns.len(), 1);
        assert_eq!(session.turns[0].prompt, "prompt test");
        assert_eq!(session.turns[0].assistant_response, "response test");
    }

    #[test]
    fn test_add_turn_without_session() {
        let (mut manager, _dir) = make_manager();
        let result = manager.add_turn("test", "test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_save_and_list() {
        let (mut manager, _dir) = make_manager();
        manager.create("model1", &["skill1".to_string()]).unwrap();
        manager.add_turn("prompt1", "response1").unwrap();

        let list = manager.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].model, "model1");
        assert!(list[0].skills.contains("skill1"));
        assert_eq!(list[0].turn_count, 1);
    }

    #[test]
    fn test_list_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(dir.path());
        let list = manager.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_list_nonexistent_dir() {
        let manager = SessionManager::new(Path::new("/tmp/nonexistent_sessions_12345"));
        let list = manager.list().unwrap();
        assert!(list.is_empty());
    }

    #[test]
    fn test_show_session() {
        let (mut manager, _dir) = make_manager();
        manager.create("test-model", &[]).unwrap();
        let id = manager.current().unwrap().id.clone();
        manager.add_turn("prompt", "response").unwrap();

        let content = manager.show(&id).unwrap();
        assert!(content.is_some());
        assert!(content.unwrap().contains("test-model"));
    }

    #[test]
    fn test_show_nonexistent() {
        let (manager, _dir) = make_manager();
        let content = manager.show("nonexistent-id").unwrap();
        assert!(content.is_none());
    }

    #[test]
    fn test_export_session() {
        let (mut manager, dir) = make_manager();
        manager.create("test-model", &[]).unwrap();
        let id = manager.current().unwrap().id.clone();
        manager.add_turn("prompt", "response").unwrap();

        let output = dir.path().join("export.md");
        let exported = manager.export(&id, &output).unwrap();
        assert!(exported);
        assert!(output.exists());
    }

    #[test]
    fn test_export_nonexistent() {
        let (manager, dir) = make_manager();
        let output = dir.path().join("export.md");
        let exported = manager.export("nonexistent", &output).unwrap();
        assert!(!exported);
    }

    #[test]
    fn test_clean_old_sessions() {
        let (mut manager, _dir) = make_manager();
        manager.create("test", &[]).unwrap();
        manager.add_turn("p", "r").unwrap();

        let removed = manager.clean(0).unwrap();
        assert_eq!(removed, 0);
    }

    #[test]
    fn test_current_after_create() {
        let (mut manager, _dir) = make_manager();
        assert!(manager.current().is_none());
        manager.create("test", &[]).unwrap();
        assert!(manager.current().is_some());
    }

    #[test]
    fn test_save_current_persists() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut manager = SessionManager::new(dir.path());
            manager.create("model", &[]).unwrap();
            let id = manager.current().unwrap().id.clone();
            manager.add_turn("prompt", "response").unwrap();
            manager.save_current().unwrap();

            let path = dir.path().join(format!("{}.md", id));
            assert!(path.exists());
            let content = std::fs::read_to_string(path).unwrap();
            assert!(content.contains("model"));
            assert!(content.contains("prompt"));
        }
    }

    #[test]
    fn test_multiple_sessions_list() {
        let dir = tempfile::tempdir().unwrap();
        {
            let mut m1 = SessionManager::new(dir.path());
            m1.create("model-a", &[]).unwrap();
            m1.add_turn("p1", "r1").unwrap();
        }
        {
            let mut m2 = SessionManager::new(dir.path());
            m2.create("model-b", &[]).unwrap();
            m2.add_turn("p2", "r2").unwrap();
        }

        let manager = SessionManager::new(dir.path());
        let list = manager.list().unwrap();
        assert_eq!(list.len(), 2);
    }
}
