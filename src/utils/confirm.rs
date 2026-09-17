use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfirmChoice {
    Yes,
    No,
    All,
    Abort,
}

pub fn confirm_tool(tool_name: &str, description: &str, is_destructive: bool) -> ConfirmChoice {
    let label = if is_destructive {
        format!("🔴 {}: {} [Y/n/a/b]", tool_name, description)
    } else {
        format!("  {}: {} [Y/n/a/b]", tool_name, description)
    };

    loop {
        print!("{} ", label);
        io::stdout().flush().expect("flush");

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("read line");
        let input = input.trim().to_lowercase();

        match input.as_str() {
            "" | "y" | "yes" | "s" | "si" => return ConfirmChoice::Yes,
            "n" | "no" => return ConfirmChoice::No,
            "a" | "all" | "t" | "todos" => return ConfirmChoice::All,
            "b" | "abort" => return ConfirmChoice::Abort,
            _ => continue,
        }
    }
}

#[allow(dead_code)]
pub fn confirm_plan(plan: &[(&str, &str, bool)], batch: bool) -> bool {
    if plan.is_empty() {
        return true;
    }

    println!("\n📋 Plan de ejecución:");
    for (i, (tool, description, destructive)) in plan.iter().enumerate() {
        let icon = if *destructive { "🔴" } else { "  " };
        println!("  {}. {} {} - {}", i + 1, icon, tool, description);
    }

    if batch {
        print!("\n¿Ejecutar todo? [Y/n]: ");
    } else {
        print!("\n¿Continuar? [Y/n]: ");
    }
    io::stdout().flush().expect("flush");

    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("read line");
    matches!(input.trim().to_lowercase().as_str(), "" | "y" | "yes" | "s" | "si")
}

#[allow(dead_code)]
pub fn confirm_destructive(action: &str) -> bool {
    println!("\n⚠️  ACCIÓN DESTRUCTIVA DETECTADA ⚠️");
    println!("  {}", action);
    print!("¿Estás seguro? [y/N]: ");
    io::stdout().flush().expect("flush");

    let mut input = String::new();
    io::stdin().read_line(&mut input).expect("read line");
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes" | "s" | "si")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confirm_destructive_detection() {
        let result = confirm_destructive("rm -rf /");
        assert!(!result);
    }
}
