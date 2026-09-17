use crate::agent::skills::SkillKind;
use crate::cli::GlobalFlags;
use crate::config::Config;

pub struct ModelRouter {
    config: Config,
}

impl ModelRouter {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.clone(),
        }
    }

    pub fn select(&self, prompt: &str, flags: &GlobalFlags) -> (String, SkillKind) {
        if flags.reasoning {
            return (self.config.models.reasoning.clone(), SkillKind::Generic);
        }

        if flags.fast {
            return (self.config.models.simple.clone(), SkillKind::Generic);
        }

        if self.is_simple_query(prompt) {
            return (self.config.models.simple.clone(), SkillKind::Generic);
        }

        (self.config.models.complex.clone(), SkillKind::Generic)
    }

    pub fn supports_tools(&self, model: &str) -> bool {
        model == self.config.models.complex
    }

    fn is_simple_query(&self, prompt: &str) -> bool {
        let simple_keywords = [
            "qué es", "define", "explica", "qué significa",
            "dime", "adiós", "gracias",
        ];

        let action_keywords = [
            "crea", "creas", "creo", "crean", "crees", "cree", "creen",
            "crear", "creado",
            "elimina", "eliminas", "elimino", "eliminan", "elimines",
            "elimine", "eliminen", "eliminar",
            "borra", "borras", "borro", "borran", "borres", "borre", "borren",
            "instala", "instalas", "instalo", "instalan", "instales",
            "instale", "instalen", "instalar",
            "configura", "configuras", "configuro", "configuran",
            "configures", "configure", "configuren", "configurar",
            "actualiza", "actualizas", "actualizo", "actualizan",
            "actualices", "actualice", "actualicen", "actualizar",
            "ejecuta", "ejecutas", "ejecuto", "ejecutan", "ejecutes",
            "ejecute", "ejecuten", "ejecutar",
            "corre", "corras", "corro", "corran", "correr",
            "busca", "buscas", "busco", "buscan", "busques", "busque",
            "busquen", "buscar",
            "encuentra", "encuentras", "encuentro", "encuentran",
            "encuentres", "encuentre", "encuentren", "encontrar",
            "modifica", "modificas", "modifico", "modifican", "modifiques",
            "modifique", "modifiquen", "modificar",
            "edita", "editas", "edito", "editan", "edites", "edite",
            "editen", "editar",
            "escribe", "escribes", "escribo", "escriben", "escribas",
            "escriba", "escriban", "escribir",
            "lee", "lees", "leo", "leen", "leer",
            "analiza", "analizas", "analizo", "analizan", "analices",
            "analice", "analicen", "analizar",
            "genera", "generas", "genero", "generan", "generes",
            "genere", "generen", "generar",
            "desarrolla", "desarrollas", "desarrollo", "desarrollan",
            "desarrolles", "desarrolle", "desarrollen", "desarrollar",
            "compila", "compilas", "compilo", "compilan", "compiles",
            "compile", "compilen", "compilar",
            "construye", "construyes", "construyo", "construyen",
            "construyas", "construya", "construyan", "construir",
            "formatea", "formateas", "formateo", "formatean", "formatees",
            "formatee", "formateen", "formatear",
            "descarga", "descargas", "descargo", "descorgan",
            "descargues", "descargue", "descarguen", "descargar",
            "sube", "subes", "subo", "suben", "subir",
            "inicia", "inicias", "inicio", "inician", "inicies",
            "inicie", "inicien", "iniciar",
            "limpia", "limpias", "limpio", "limpian", "limpie",
            "limpien", "limpiar",
            "detén", "detenga", "detengan", "detener",
            "necesito", "necesitas", "necesita", "necesitan",
            "quiero", "quieres", "quiere", "quieren",
            "puedes", "puede", "pueden", "podrías", "podría",
            "haz", "haga", "hagan", "hacer",
        ];

        let lower = prompt.to_lowercase();
        let has_simple = simple_keywords.iter().any(|kw| lower.contains(kw));
        let has_hola = lower == "hola" || lower.starts_with("hola ") || lower.ends_with(" hola") || lower.contains(" hola ");
        let has_simple = has_simple || has_hola;
        let has_action = action_keywords.iter().any(|kw| lower.contains(kw));

        has_simple && !has_action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::GlobalFlags;

    fn make_flags(reasoning: bool, fast: bool) -> GlobalFlags {
        GlobalFlags {
            batch: false,
            reasoning,
            fast,
            dry_run: false,
            destructive: false,
            daemon: None,
            checkpoint: false,
        }
    }

    fn make_router() -> ModelRouter {
        ModelRouter::new(&Config::default())
    }

    #[test]
    fn test_select_reasoning_flag() {
        let router = make_router();
        let flags = make_flags(true, false);
        let (model, _kind) = router.select("cualquier cosa", &flags);
        assert_eq!(model, "deepseek-r1:8b");
    }

    #[test]
    fn test_select_fast_flag() {
        let router = make_router();
        let flags = make_flags(false, true);
        let (model, _kind) = router.select("tarea compleja", &flags);
        assert_eq!(model, "gemma3:4b");
    }

    #[test]
    fn test_reasoning_overrides_fast() {
        let router = make_router();
        let flags = make_flags(true, true);
        let (model, _kind) = router.select("cualquier cosa", &flags);
        assert_eq!(model, "deepseek-r1:8b");
    }

    #[test]
    fn test_simple_query_uses_simple_model() {
        let router = make_router();
        let flags = make_flags(false, false);
        let (model, _kind) = router.select("qué es SOLID", &flags);
        assert_eq!(model, "gemma3:4b");
    }

    #[test]
    fn test_complex_query_uses_complex_model() {
        let router = make_router();
        let flags = make_flags(false, false);
        let (model, _kind) = router.select("actualiza repositorios y configura el firewall", &flags);
        assert_eq!(model, "qwen2.5:3b");
    }

    #[test]
    fn test_complex_query_with_uppercase() {
        let router = make_router();
        let flags = make_flags(false, false);
        let (model, _kind) = router.select("ACTUALIZA repositorios", &flags);
        assert_eq!(model, "qwen2.5:3b");
    }

    #[test]
    fn test_is_simple_query_detects_keywords() {
        let router = make_router();
        assert!(router.is_simple_query("qué es rust"));
        assert!(router.is_simple_query("define polimorfismo"));
        assert!(router.is_simple_query("explica SOLID"));
        assert!(router.is_simple_query("hola mundo"));
        assert!(router.is_simple_query("dime qué es rust"));
    }

    #[test]
    fn test_supports_tools_complex_model() {
        let router = make_router();
        assert!(router.supports_tools("qwen2.5:3b"));
    }

    #[test]
    fn test_supports_tools_simple_model() {
        let router = make_router();
        assert!(!router.supports_tools("gemma3:4b"));
    }

    #[test]
    fn test_supports_tools_reasoning_model() {
        let router = make_router();
        assert!(!router.supports_tools("deepseek-r1:8b"));
    }

    #[test]
    fn test_is_simple_query_rejects_complex() {
        let router = make_router();
        assert!(!router.is_simple_query("actualiza repositorios"));
        assert!(!router.is_simple_query("configura el servidor web"));
        assert!(!router.is_simple_query("analiza la arquitectura del proyecto"));
    }

    #[test]
    fn test_holamundo_not_simple() {
        let router = make_router();
        assert!(!router.is_simple_query("desarrolla un holamundo.py"));
        assert!(!router.is_simple_query("genera un holamundo.py"));
    }

    #[test]
    fn test_hola_alone_is_simple() {
        let router = make_router();
        assert!(router.is_simple_query("hola mundo"));
        assert!(router.is_simple_query("hola"));
        assert!(router.is_simple_query("dime hola"));
    }

    #[test]
    fn test_conjugated_verbs_detected() {
        let router = make_router();
        assert!(!router.is_simple_query("necesito que crees el archivo"));
        assert!(!router.is_simple_query("quiero que desarrolles el programa"));
        assert!(!router.is_simple_query("puedes generar un archivo"));
        assert!(!router.is_simple_query("necesito borrar un archivo"));
        assert!(!router.is_simple_query("haz un directorio"));
    }

    #[test]
    fn test_simple_requests_remain_simple() {
        let router = make_router();
        assert!(router.is_simple_query("qué es rust"));
        assert!(router.is_simple_query("explica SOLID"));
        assert!(router.is_simple_query("gracias"));
    }

    #[test]
    fn test_select_returns_skillkind() {
        let router = make_router();
        let flags = make_flags(false, false);
        let (_model, kind) = router.select("actualiza repositorios", &flags);
        assert_eq!(kind, crate::agent::skills::SkillKind::Generic);
    }

    #[test]
    fn test_select_fast_returns_generic_kind() {
        let router = make_router();
        let flags = make_flags(false, true);
        let (_model, kind) = router.select("cualquier cosa", &flags);
        assert_eq!(kind, crate::agent::skills::SkillKind::Generic);
    }
}
