use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkillKind {
    Linux,
    Docker,
    Python,
    Cobol,
    PowerBI,
    Kubernetes,
    PostgreSQL,
    Security,
    Git,
    Generic,
}

impl SkillKind {
    pub fn from_domain(domain: &str) -> Self {
        match domain.to_lowercase().as_str() {
            "linux" => SkillKind::Linux,
            "docker" => SkillKind::Docker,
            "python" => SkillKind::Python,
            "cobol" => SkillKind::Cobol,
            "powerbi" => SkillKind::PowerBI,
            "kubernetes" | "k8s" => SkillKind::Kubernetes,
            "postgresql" | "postgres" => SkillKind::PostgreSQL,
            "security" | "seguridad" => SkillKind::Security,
            "git" => SkillKind::Git,
            _ => SkillKind::Generic,
        }
    }
}

const STOP_WORDS: &[&str] = &[
    "un", "una", "unos", "unas", "el", "la", "los", "las",
    "de", "del", "en", "con", "por", "para", "a", "al",
    "y", "e", "o", "u", "que", "es", "se", "no",
    "su", "sus", "tu", "mi", "te", "le", "lo",
    "como", "mas", "pero", "sin", "entre", "todo",
    "este", "esta", "ese", "esa", "esto", "eso",
    "haz", "hace", "hacer", "puede", "debe",
    "voy", "vas", "va", "vamos", "van",
    "he", "has", "ha", "hemos", "han",
    "soy", "eres", "somos", "son",
    "estoy", "estas", "esta", "estamos", "estan",
];

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub name: String,
    pub description: String,
    pub content: String,
    pub domain: String,
}

impl Skill {
    pub fn kind(&self) -> SkillKind {
        SkillKind::from_domain(&self.domain)
    }
}

pub struct SkillsLoader {
    skills: Vec<Skill>,
}

impl SkillsLoader {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    pub fn load_from(dir: &Path) -> Result<Self> {
        let mut skills = Vec::new();

        if !dir.exists() {
            return Ok(Self { skills });
        }

        Self::load_recursive(dir, "", &mut skills)?;

        Ok(Self { skills })
    }

    fn load_recursive(dir: &Path, domain: &str, skills: &mut Vec<Skill>) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let skill_path = path.join("SKILL.md");
            if skill_path.exists() {
                let content = match std::fs::read_to_string(&skill_path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                if let Some(mut skill) = Self::parse_skill(&content) {
                    let d = if domain.is_empty() { "generic" } else { domain };
                    skill.domain = d.to_string();
                    tracing::info!("Skill cargada: {} (dominio: {})", skill.name, skill.domain);
                    skills.push(skill);
                }
            } else {
                let subdomain = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                Self::load_recursive(&path, &subdomain, skills)?;
            }
        }
        Ok(())
    }

    fn parse_skill(content: &str) -> Option<Skill> {
        let content = content.trim();
        if !content.starts_with("---") {
            return None;
        }

        let rest = &content[3..];
        let end = rest.find("---")?;
        let frontmatter_str = &rest[..end];
        let body = rest[end + 3..].trim();

        let name = Self::extract_field(frontmatter_str, "name")?;
        let description = Self::extract_field(frontmatter_str, "description")?;

        Some(Skill {
            name,
            description,
            content: body.to_string(),
            domain: String::new(),
        })
    }

    fn extract_field(frontmatter: &str, field: &str) -> Option<String> {
        for line in frontmatter.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix(&format!("{}:", field)) {
                let value = stripped.trim().trim_matches('"').to_string();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
        None
    }

    pub fn all(&self) -> &[Skill] {
        &self.skills
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.name == name)
    }

    pub fn match_prompt(&self, prompt: &str) -> Vec<(&Skill, SkillKind)> {
        let lower = prompt.to_lowercase();
        let mut scored: Vec<(f64, &Skill, SkillKind)> = self
            .skills
            .iter()
            .map(|skill| {
                let score = self.compute_relevance(&lower, skill);
                let kind = skill.kind();
                (score, skill, kind)
            })
            .collect();

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .into_iter()
            .take(3)
            .filter(|(s, _, _)| *s >= 3.0)
            .map(|(_, s, k)| (s, k))
            .collect()
    }

    fn compute_relevance(&self, prompt: &str, skill: &Skill) -> f64 {
        let mut score = 0.0;

        let desc_lower = skill.description.to_lowercase();
        let name_lower = skill.name.to_lowercase();
        let content_lower = skill.content.to_lowercase();

        let prompt_words: Vec<&str> = prompt.split_whitespace()
            .filter(|w| !is_stop_word(w) && w.len() > 2)
            .collect();
        let desc_words: Vec<&str> = desc_lower.split_whitespace()
            .filter(|w| !is_stop_word(w) && w.len() > 2)
            .collect();

        if prompt_words.is_empty() {
            return 0.0;
        }

        let category = name_lower.split('-').next().unwrap_or("");

        if prompt.contains("codigo") || prompt.contains("código") || prompt.contains("arquitectura")
            || prompt.contains("testing") || prompt.contains("seguro") || prompt.contains("estandar")
            || prompt.contains("estándar") || prompt.contains("solid")
        {
            if category == "a" { score += 3.0; }
        }
        if prompt.contains("autenticacion") || prompt.contains("autenticación")
            || prompt.contains("seguridad") || prompt.contains("login")
            || prompt.contains("html") || prompt.contains("css") || prompt.contains("ui")
            || prompt.contains("componente") || prompt.contains("javascript")
        {
            if category == "b" { score += 3.0; }
        }
        if prompt.contains("base de datos") || prompt.contains("database") || prompt.contains("sql")
            || prompt.contains("dexie") || prompt.contains("debug")
            || prompt.contains("documentacion") || prompt.contains("documentación")
            || prompt.contains("admin") || prompt.contains("sistema") || prompt.contains("linux")
        {
            if category == "c" { score += 3.0; }
        }
        if prompt.contains("erp") || prompt.contains("git") || prompt.contains("prompt")
            || prompt.contains("workflow") || prompt.contains("offline")
        {
            if category == "d" { score += 3.0; }
        }

        for word in &prompt_words {
            if desc_words.contains(word) {
                score += 1.0;
            }
            if content_lower.contains(word) {
                score += 0.5;
            }
        }

        let prompt_phrases: Vec<String> = prompt_words
            .windows(2)
            .map(|w| w.join(" "))
            .collect();

        for phrase in &prompt_phrases {
            if desc_lower.contains(phrase) {
                score += 3.0;
            }
        }

        if prompt.contains(&name_lower) {
            score += 10.0;
        }

        score
    }

    pub fn format_skills_for_prompt(&self, skills: &[&Skill]) -> String {
        if skills.is_empty() {
            return String::new();
        }

        let mut output = String::from("\n\n## SKILLS ACTIVAS\n\n");
        for skill in skills {
            output.push_str(&format!("### {}\n", skill.name));
            output.push_str(&format!("{}\n\n", skill.description));
            output.push_str(&format!("{}", skill.content));
            output.push_str("\n\n---\n\n");
        }
        output
    }
}

impl Default for SkillsLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn create_test_skill(dir: &Path, name: &str, description: &str, body: &str) {
        let skill_dir = dir.join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let path = skill_dir.join("SKILL.md");
        std::fs::write(
            &path,
            format!("---\nname: {}\ndescription: {}\n---\n\n{}", name, description, body),
        )
        .unwrap();
    }

    fn create_domain_skill(dir: &Path, domain: &str, name: &str, description: &str, body: &str) {
        let skill_dir = dir.join(domain).join(name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        let path = skill_dir.join("SKILL.md");
        std::fs::write(
            &path,
            format!("---\nname: {}\ndescription: {}\n---\n\n{}", name, description, body),
        )
        .unwrap();
    }

    #[test]
    fn test_parse_skill() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(dir.path(), "test-skill", "A test skill for unit testing", "# Test\n\nContent here.");

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert_eq!(loader.len(), 1);
        let skill = &loader.all()[0];
        assert_eq!(skill.name, "test-skill");
        assert_eq!(skill.domain, "generic");
        assert_eq!(skill.kind(), SkillKind::Generic);
        assert!(skill.content.contains("Content"));
    }

    #[test]
    fn test_parse_skill_without_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("noskill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "No frontmatter here").unwrap();

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert_eq!(loader.len(), 0);
    }

    #[test]
    fn test_load_from_nonexistent_dir() {
        let loader = SkillsLoader::load_from(Path::new("/tmp/nonexistent_skills_12345")).unwrap();
        assert_eq!(loader.len(), 0);
    }

    #[test]
    fn test_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert_eq!(loader.len(), 0);
    }

    #[test]
    fn test_find_by_name() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(dir.path(), "coding-standards", "Coding standards", "Content");

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert!(loader.find_by_name("coding-standards").is_some());
        assert!(loader.find_by_name("nonexistent").is_none());
    }

    #[test]
    fn test_match_prompt_keyword() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(
            dir.path(),
            "c-sysadmin",
            "Administración profesional de sistemas Linux - mantenimiento, seguridad",
            "# Linux Admin\nCommands: apt, systemctl, journalctl",
        );
        create_test_skill(
            dir.path(),
            "c-database-design-sql",
            "Diseñar bases de datos relacionales escalables",
            "# Database\nSQL, PostgreSQL, MySQL",
        );

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        let matched = loader.match_prompt("administra el sistema linux y configura el firewall");
        assert!(!matched.is_empty());
        assert_eq!(matched[0].0.name, "c-sysadmin");

        let matched = loader.match_prompt("crea una base de datos en postgresql");
        assert!(!matched.is_empty());
        assert_eq!(matched[0].0.name, "c-database-design-sql");
    }

    #[test]
    fn test_match_prompt_empty() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(dir.path(), "test", "Description", "Content");
        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert!(loader.match_prompt("").is_empty());
    }

    #[test]
    fn test_match_prompt_no_match() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(dir.path(), "linux-admin", "Linux administration", "Content");
        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert!(loader.match_prompt("que es la teoria de cuerdas en fisica").is_empty());
    }

    #[test]
    fn test_load_skills_skip_non_skill_dirs() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(dir.path(), "valid-skill", "A valid skill", "Content");
        let non_skill = dir.path().join(".hidden");
        std::fs::create_dir_all(&non_skill).unwrap();
        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert_eq!(loader.len(), 1);
    }

    #[test]
    fn test_parse_skill_with_quoted_fields() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("quoted-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: \"quoted-name\"\ndescription: \"A description\"\n---\n\nBody",
        )
        .unwrap();

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        let skill = loader.find_by_name("quoted-name");
        assert!(skill.is_some());
        assert_eq!(skill.unwrap().description, "A description");
    }

    #[test]
    fn test_format_skills_for_prompt() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(
            dir.path(),
            "test-skill",
            "Test description",
            "Skill content here",
        );

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        let matched = loader.match_prompt("test-skill description");
        let skills: Vec<&Skill> = matched.into_iter().map(|(s, _)| s).collect();
        let formatted = loader.format_skills_for_prompt(&skills);
        assert!(formatted.contains("test-skill"));
        assert!(formatted.contains("Test description"));
        assert!(formatted.contains("Skill content here"));
    }

    #[test]
    fn test_format_skills_empty() {
        let loader = SkillsLoader::new();
        let formatted = loader.format_skills_for_prompt(&[]);
        assert!(formatted.is_empty());
    }

    #[test]
    fn test_skill_kind_from_domain() {
        assert_eq!(SkillKind::from_domain("linux"), SkillKind::Linux);
        assert_eq!(SkillKind::from_domain("docker"), SkillKind::Docker);
        assert_eq!(SkillKind::from_domain("python"), SkillKind::Python);
        assert_eq!(SkillKind::from_domain("postgresql"), SkillKind::PostgreSQL);
        assert_eq!(SkillKind::from_domain("k8s"), SkillKind::Kubernetes);
        assert_eq!(SkillKind::from_domain("unknown"), SkillKind::Generic);
    }

    #[test]
    fn test_skill_kind_equality() {
        assert_eq!(SkillKind::Linux, SkillKind::Linux);
        assert_ne!(SkillKind::Linux, SkillKind::Docker);
    }

    #[test]
    fn test_match_prompt_returns_skill_kind_from_domain() {
        let dir = tempfile::tempdir().unwrap();
        create_domain_skill(
            dir.path(),
            "linux",
            "C-sysadmin",
            "Linux administration and system management",
            "# Linux\nCommands for linux system",
        );

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        let matched = loader.match_prompt("administra el sistema linux");
        assert!(!matched.is_empty(), "should match C-sysadmin skill");
        let (_skill, kind) = matched[0];
        assert_eq!(kind, SkillKind::Linux, "skill under linux/ dir should map to Linux kind");
    }

    #[test]
    fn test_skill_kind_matches_domain_directory() {
        let dir = tempfile::tempdir().unwrap();
        create_domain_skill(dir.path(), "linux", "linux-admin", "Linux administration", "Content");
        create_domain_skill(dir.path(), "git", "git-workflow", "Git workflow", "Content");
        create_domain_skill(dir.path(), "security", "secure-coding", "Security coding", "Content");

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert_eq!(loader.len(), 3);

        let linux = loader.find_by_name("linux-admin").unwrap();
        assert_eq!(linux.domain, "linux");
        assert_eq!(linux.kind(), SkillKind::Linux);

        let git = loader.find_by_name("git-workflow").unwrap();
        assert_eq!(git.domain, "git");
        assert_eq!(git.kind(), SkillKind::Git);

        let security = loader.find_by_name("secure-coding").unwrap();
        assert_eq!(security.domain, "security");
        assert_eq!(security.kind(), SkillKind::Security);
    }

    #[test]
    fn test_load_recursive_two_levels_deep() {
        let dir = tempfile::tempdir().unwrap();
        create_domain_skill(dir.path(), "linux", "C-sysadmin", "Linux admin", "Content here");
        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        assert_eq!(loader.len(), 1);
        let skill = &loader.all()[0];
        assert_eq!(skill.name, "C-sysadmin");
        assert_eq!(skill.domain, "linux");
        assert_eq!(skill.kind(), SkillKind::Linux);
    }

    #[test]
    fn test_multiple_skills_match_returns_multiple_kinds() {
        let dir = tempfile::tempdir().unwrap();
        create_test_skill(
            dir.path(),
            "docker-admin",
            "Docker container management",
            "# Docker\nManage containers",
        );
        create_test_skill(
            dir.path(),
            "python-dev",
            "Python development",
            "# Python\nWrite python code",
        );

        let loader = SkillsLoader::load_from(dir.path()).unwrap();
        let matched = loader.match_prompt("docker and python development");
        assert!(matched.len() <= 3);
    }
}