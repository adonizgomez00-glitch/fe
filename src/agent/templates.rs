use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppType {
    Cli,
    Desktop,
    Web,
    Admin,
    Doc,
}

#[derive(Debug, Clone)]
pub struct AppFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct AppTemplate {
    pub dir: String,
    pub files: Vec<AppFile>,
    pub description: String,
}

pub fn detect_app_type(description: &str) -> AppType {
    let d = description.to_lowercase();
    if d.contains("cli")
        || d.contains("terminal")
        || d.contains("comando")
        || d.contains("argumento")
        || d.contains("subcomando")
    {
        AppType::Cli
    } else if d.contains("web")
        || d.contains("api")
        || d.contains("servidor")
        || d.contains("http")
        || d.contains("flask")
        || d.contains("express")
        || d.contains("servicio web")
        || d.contains("api rest")
        || d.contains("rest api")
        || d.contains("restful")
    {
        AppType::Web
    } else if d.contains("escritorio")
        || d.contains("gui")
        || d.contains("ventana")
        || d.contains("tkinter")
        || d.contains("desktop")
        || d.contains("interfaz grafica")
        || d.contains("pyside")
        || d.contains("qt")
    {
        AppType::Desktop
    } else if d.contains("admin")
        || d.contains("sistema")
        || d.contains("respaldo")
        || d.contains("backup")
        || d.contains("monitoreo")
        || d.contains("servicio")
        || d.contains("daemon")
        || d.contains("log")
    {
        AppType::Admin
    } else if d.contains("doc")
        || d.contains("reporte")
        || d.contains("informe")
        || d.contains("pdf")
        || d.contains("markdown")
        || d.contains("html")
    {
        AppType::Doc
    } else {
        AppType::Cli
    }
}

pub fn generate_app(language: &str, app_type: AppType, name: &str, description: &str) -> AppTemplate {
    let name = if name.is_empty() {
        format!("app_{}", language)
    } else {
        name.to_string()
    };

    let (dir, files, desc) = match (language, app_type) {
        ("python", AppType::Cli) => python_cli(&name, description),
        ("python", AppType::Desktop) => python_desktop(&name, description),
        ("python", AppType::Web) => python_web(&name, description),
        ("python", AppType::Admin) => python_admin(&name, description),
        ("python", AppType::Doc) => python_doc(&name, description),
        ("node", AppType::Cli) => node_cli(&name, description),
        ("node", AppType::Web) => node_web(&name, description),
        ("node", AppType::Desktop) => node_desktop(&name, description),
        ("node", _) => node_cli(&name, description),
        ("rust", AppType::Cli) => rust_cli(&name, description),
        ("rust", AppType::Web) => rust_web(&name, description),
        ("rust", _) => rust_cli(&name, description),
        ("bash", AppType::Cli) => bash_cli(&name, description),
        ("bash", AppType::Admin) => bash_admin(&name, description),
        ("bash", _) => bash_cli(&name, description),
        _ => python_cli(&name, description),
    };

    AppTemplate { dir, files, description: desc }
}

fn quote_sh(s: &str) -> String {
    if s.contains(char::is_whitespace) || s.contains(['\'', '"', '$', '`', '\\']) {
        format!("'{}'", s.replace('\'', "'\\''"))
    } else {
        s.to_string()
    }
}

pub fn project_to_commands(template: &AppTemplate) -> Vec<(String, String)> {
    let mut commands = Vec::new();
    let dir = quote_sh(&template.dir);

    let mut script = format!("mkdir -p {}\n", dir);
    for (i, file) in template.files.iter().enumerate() {
        let full_path = format!("{}/{}", template.dir, file.path);
        let parent = Path::new(&full_path).parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let heredoc_id = format!("TPL_END_{}", i);
        script.push_str(&format!(
            "mkdir -p {} && cat > {} << '{}'\n{}\n{}\n",
            quote_sh(&parent),
            quote_sh(&full_path),
            heredoc_id,
            file.content,
            heredoc_id,
        ));
    }

    commands.push((template.description.clone(), script));
    commands
}

pub fn normalize_language(lang: &str) -> &'static str {
    match lang {
        "python" | "py" => "python",
        "node" | "js" | "javascript" => "node",
        "rust" | "rs" => "rust",
        "bash" | "sh" | "shell" => "bash",
        _ => "python",
    }
}

#[allow(dead_code)]
pub fn extension(language: &str) -> &'static str {
    match language {
        "python" => "py",
        "node" => "js",
        "rust" => "rs",
        "bash" => "sh",
        _ => "py",
    }
}

fn replace_name(template: &str, name: &str) -> String {
    template.replace("__NAME__", name)
        .replace("__DIR__", name)
}

// ─── Python generators ─────────────────────────────────────────────

fn python_cli(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let main_content = replace_name(r#"#!/usr/bin/env python3
import argparse
import logging
import sys


def setup_logging(verbose: bool = False) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    logging.basicConfig(
        level=level,
        format="%(asctime)s [%(levelname)s] %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )


def process(args: argparse.Namespace) -> int:
    logging.debug(f"Argumentos: {args}")
    if hasattr(args, "input") and args.input:
        print(args.input)
        return 0
    print(f"Hello from __NAME__")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="__NAME__")
    parser.add_argument("input", nargs="?", help="Input argument")
    parser.add_argument("-v", "--verbose", action="store_true", help="Verbose output")
    args = parser.parse_args(argv)
    setup_logging(args.verbose)
    return process(args)


if __name__ == "__main__":
    sys.exit(main())
"#, name);

    let test_content = replace_name(r#"""Tests for __NAME__"""
from __NAME__ import main


def test_main_returns_zero():
    assert main([]) == 0


def test_main_with_input():
    assert main(["test_input"]) == 0
"#, name);

    let files = vec![
        AppFile { path: format!("{}.py", name), content: main_content },
        AppFile { path: "requirements.txt".into(), content: "# No external dependencies\n".into() },
        AppFile { path: "tests/__init__.py".into(), content: "".into() },
        AppFile { path: format!("tests/test_{}.py", name), content: test_content },
    ];

    (name.to_string(), files, format!("App Python CLI en {}/", name))
}

fn python_desktop(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let content = replace_name(r#"#!/usr/bin/env python3
import tkinter as tk
from tkinter import ttk, messagebox
import sys


class App:
    def __init__(self, root: tk.Tk) -> None:
        self.root = root
        self.root.title("__NAME__")
        self.root.geometry("600x400")
        self.root.minsize(400, 300)
        self._build_menu()
        self._build_ui()

    def _build_menu(self) -> None:
        menubar = tk.Menu(self.root)
        file_menu = tk.Menu(menubar, tearoff=0)
        file_menu.add_command(label="Salir", command=self.root.quit, accelerator="Ctrl+Q")
        menubar.add_cascade(label="Archivo", menu=file_menu)
        help_menu = tk.Menu(menubar, tearoff=0)
        help_menu.add_command(label="Acerca de", command=self._show_about)
        menubar.add_cascade(label="Ayuda", menu=help_menu)
        self.root.config(menu=menubar)
        self.root.bind("<Control-q>", lambda e: self.root.quit())

    def _build_ui(self) -> None:
        mainframe = ttk.Frame(self.root, padding="10")
        mainframe.pack(fill=tk.BOTH, expand=True)
        ttk.Label(mainframe, text="__NAME__", font=("Helvetica", 16)).pack(pady=10)
        ttk.Button(mainframe, text="Salir", command=self.root.quit).pack(pady=5)

    def _show_about(self) -> None:
        messagebox.showinfo("Acerca de", "__NAME__ v1.0\n\nHecho con Python y tkinter")


def main() -> None:
    root = tk.Tk()
    App(root)
    root.mainloop()


if __name__ == "__main__":
    main()
"#, name);

    let files = vec![
        AppFile { path: format!("{}.py", name), content },
    ];

    (name.to_string(), files, format!("App Python Desktop (tkinter) en {}/", name))
}

fn python_web(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let content = replace_name(r#"#!/usr/bin/env python3
import http.server
import json
import sys
from datetime import datetime
from urllib.parse import urlparse

PORT = 8080


class AppHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self) -> None:
        parsed = urlparse(self.path)
        if parsed.path == "/":
            self._html(f"""<!DOCTYPE html>
<html lang="es">
<head><meta charset="utf-8"><title>__NAME__</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
  body { font-family: sans-serif; max-width: 800px; margin: 2em auto; padding: 0 1em; }
  h1 { color: #333; }
</style></head>
<body>
<h1>__NAME__</h1>
<p>Bienvenido.</p>
<p><a href="/api/time">Hora actual (JSON)</a></p>
</body></html>""")
        elif parsed.path == "/api/time":
            self._json({"time": datetime.now().isoformat(), "status": "ok"})
        else:
            self._json({"error": "not found"}, 404)

    def do_POST(self) -> None:
        content_len = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(content_len).decode() if content_len else "{}"
        try:
            data = json.loads(body)
            self._json({"received": data, "status": "ok"})
        except json.JSONDecodeError:
            self._json({"error": "invalid json"}, 400)

    def _html(self, html: str) -> None:
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.end_headers()
        self.wfile.write(html.encode())

    def _json(self, data: dict, status: int = 200) -> None:
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data, ensure_ascii=False).encode())

    def log_message(self, fmt: str, *args: tuple) -> None:
        sys.stderr.write(f"[{datetime.now().isoformat()}] {self.address_string()} - {fmt % args}\n")


def main() -> None:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else PORT
    server = http.server.HTTPServer(("0.0.0.0", port), AppHandler)
    print(f"Servidor en http://localhost:{port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nDetenido.")
        server.server_close()


if __name__ == "__main__":
    main()
"#, name);

    let files = vec![
        AppFile { path: format!("{}.py", name), content },
    ];

    (name.to_string(), files, format!("App Web Python en {}/", name))
}

fn python_admin(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let main_content = replace_name(r#"#!/usr/bin/env python3
import argparse
import logging
import os
import shutil
import subprocess
import sys
from datetime import datetime


def setup_logging(log_file: str | None, verbose: bool) -> None:
    level = logging.DEBUG if verbose else logging.INFO
    handlers = [logging.StreamHandler()]
    if log_file:
        handlers.append(logging.FileHandler(log_file))
    logging.basicConfig(level=level, handlers=handlers,
                        format="%(asctime)s [%(levelname)s] %(message)s",
                        datefmt="%Y-%m-%d %H:%M:%S")


def confirm(prompt: str) -> bool:
    resp = input(f"{prompt} [y/N]: ").strip().lower()
    return resp in ("y", "yes", "s", "si")


def run_cmd(cmd: list[str], dry_run: bool = False) -> int:
    logging.info(f"Ejecutando: {' '.join(cmd)}")
    if dry_run:
        logging.info("[dry-run] omitido")
        return 0
    result = subprocess.run(cmd, capture_output=True, text=True)
    if result.stdout:
        print(result.stdout)
    if result.stderr:
        print(result.stderr, file=sys.stderr)
    return result.returncode


def run(args: argparse.Namespace) -> int:
    logging.info("Iniciando tarea administrativa")
    if not args.dry_run and not confirm("Confirmar operacion"):
        logging.warning("Cancelado por el usuario")
        return 1
    print(f"{datetime.now().isoformat()} - Operacion completada")
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Script administrativo - __NAME__")
    parser.add_argument("--dry-run", action="store_true", help="Simular sin ejecutar")
    parser.add_argument("--verbose", action="store_true", help="Salida detallada")
    parser.add_argument("--log-file", help="Archivo de log")
    args = parser.parse_args(argv)
    setup_logging(args.log_file, args.verbose)
    return run(args)


if __name__ == "__main__":
    sys.exit(main())
"#, name);

    let test_content = replace_name(r#"""Tests for __NAME__ admin script"""
from __NAME__ import main


def test_dry_run_returns_zero():
    assert main(["--dry-run"]) == 0


def test_help():
    try:
        main(["--help"])
    except SystemExit:
        pass
"#, name);

    let files = vec![
        AppFile { path: format!("{}.py", name), content: main_content },
        AppFile { path: "tests/__init__.py".into(), content: "".into() },
        AppFile { path: format!("tests/test_{}.py", name), content: test_content },
    ];

    (name.to_string(), files, format!("Script Admin Python en {}/", name))
}

fn python_doc(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let main_content = replace_name(r#"#!/usr/bin/env python3
"""Generador de documentacion / reportes."""
import argparse
import sys
from datetime import datetime


def generate_html(title: str, content: str, css: str = "") -> str:
    return f"""<!DOCTYPE html>
<html lang="es">
<head><meta charset="utf-8"><title>{title}</title>
<style>
  body { font-family: sans-serif; max-width: 900px; margin: 2em auto; padding: 0 1em; line-height: 1.6; }
  h1, h2, h3 { color: #2c3e50; }
  table { border-collapse: collapse; width: 100%; }
  th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }
  th { background: #f5f5f5; }
  .footer { margin-top: 2em; color: #888; font-size: 0.9em; }
  {css}
</style></head>
<body>
<h1>{title}</h1>
{content}
<div class="footer">Generado: {datetime.now().isoformat()}</div>
</body></html>"""


def generate_markdown(title: str, content: str) -> str:
    return f"""---
title: "{title}"
date: {datetime.now().isoformat()}
---

# {title}

{content}

---

*Generado automaticamente*
"""


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generador de reportes - __NAME__")
    parser.add_argument("title", help="Titulo del documento")
    parser.add_argument("content", help="Contenido o ruta a archivo")
    parser.add_argument("-f", "--format", choices=["html", "md"], default="html", help="Formato de salida")
    parser.add_argument("-o", "--output", help="Archivo de salida (default: stdout)")
    args = parser.parse_args(argv)
    if args.format == "md":
        doc = generate_markdown(args.title, args.content)
    else:
        doc = generate_html(args.title, args.content)
    if args.output:
        with open(args.output, "w") as f:
            f.write(doc)
        print(f"Reporte generado: {args.output}")
    else:
        print(doc)
    return 0


if __name__ == "__main__":
    sys.exit(main())
"#, name);

    let test_content = replace_name(r#"""Tests for __NAME__"""
from __NAME__ import generate_html, generate_markdown


def test_generate_html_contains_title():
    html = generate_html("Test", "<p>hello</p>")
    assert "<title>Test</title>" in html


def test_generate_markdown_includes_date():
    md = generate_markdown("Test", "content")
    assert "title: Test" in md


def test_main_returns_zero():
    from __NAME__ import main
    assert main(["Test", "content", "--format", "md"]) == 0
"#, name);

    let files = vec![
        AppFile { path: format!("{}.py", name), content: main_content },
        AppFile { path: "tests/__init__.py".into(), content: "".into() },
        AppFile { path: format!("tests/test_{}.py", name), content: test_content },
    ];

    (name.to_string(), files, format!("App Documentacion Python en {}/", name))
}

// ─── Node.js generators ────────────────────────────────────────────

fn node_cli(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let main_content = replace_name(r#"#!/usr/bin/env node
const fs = require("fs");
const path = require("path");

function printHelp() {
    console.log(`Uso: node __NAME__ [options] [input]

Opciones:
  -h, --help     Muestra esta ayuda
  -v, --version  Muestra la version
  --verbose      Salida detallada

Ejemplos:
  node __NAME__ hola
  node __NAME__ --verbose`);
}

function main(argv = process.argv.slice(2)) {
    const args = { verbose: false, input: null };
    for (let i = 0; i < argv.length; i++) {
        switch (argv[i]) {
            case "-h": case "--help": printHelp(); return 0;
            case "-v": case "--version": console.log("__NAME__ v1.0"); return 0;
            case "--verbose": args.verbose = true; break;
            default: args.input = argv[i];
        }
    }
    if (args.verbose) console.error("[verbose] Iniciando");
    if (args.input) { console.log(args.input); return 0; }
    console.log("Hello from __NAME__");
    return 0;
}

if (require.main === module) {
    process.exit(main());
}

module.exports = { main };
"#, name);

    let test_content = replace_name(r#"const { main } = require("./__NAME__");

test("main returns 0", () => {
    expect(main([])).toBe(0);
});

test("main with input", () => {
    expect(main(["test"])).toBe(0);
});
"#, name);

    let package_json = r#"{
  "name": "app_node_cli",
  "version": "1.0.0",
  "private": true,
  "scripts": {
    "start": "node cli.js",
    "test": "jest"
  },
  "devDependencies": {
    "jest": "^29.0.0"
  }
}
"#;

    let files = vec![
        AppFile { path: format!("{}.js", name), content: main_content },
        AppFile { path: "package.json".into(), content: package_json.into() },
        AppFile { path: format!("{}.test.js", name), content: test_content },
    ];

    (name.to_string(), files, format!("App Node CLI en {}/", name))
}

fn node_web(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let content = replace_name(r#"#!/usr/bin/env node
const http = require("http");

const PORT = parseInt(process.argv[2], 10) || 8080;

function serveHTML(res, body) {
    res.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
    res.end(body);
}

function serveJSON(res, data, status = 200) {
    res.writeHead(status, { "Content-Type": "application/json" });
    res.end(JSON.stringify(data, null, 2));
}

const server = http.createServer((req, res) => {
    const url = new URL(req.url, `http://${req.headers.host}`);
    if (req.method === "GET" && url.pathname === "/") {
        serveHTML(res, `<!DOCTYPE html>
<html lang="es">
<head><meta charset="utf-8"><title>__NAME__</title>
<style>body { font-family: sans-serif; max-width: 800px; margin: 2em auto; }</style>
</head>
<body><h1>__NAME__</h1><p>Bienvenido.</p></body></html>`);
    } else if (req.method === "GET" && url.pathname === "/api/health") {
        serveJSON(res, { status: "ok", uptime: process.uptime() });
    } else {
        serveJSON(res, { error: "not found" }, 404);
    }
});

server.listen(PORT, () => {
    console.log(`Servidor en http://localhost:${PORT}`);
});
"#, name);

    let files = vec![
        AppFile { path: format!("{}.js", name), content },
    ];

    (name.to_string(), files, format!("App Web Node en {}/", name))
}

fn node_desktop(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let content = replace_name(r#"#!/usr/bin/env node
const readline = require("readline");

const rl = readline.createInterface({ input: process.stdin, output: process.stdout });

function prompt(question) {
    return new Promise((resolve) => {
        rl.question(question + " ", resolve);
    });
}

async function main() {
    console.log("=== __NAME__ (Terminal UI) ===");
    const name = await prompt("Nombre:");
    console.log(`Hola, ${name}!`);
    rl.close();
}

main().catch(console.error);
"#, name);

    let files = vec![
        AppFile { path: format!("{}.js", name), content },
    ];

    (name.to_string(), files, format!("App Node Terminal en {}/", name))
}

// ─── Rust generators ───────────────────────────────────────────────

fn rust_cli(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
"#,
        name
    );

    let main_rs = replace_name(r#"use anyhow::Result;
use std::env;

fn print_help() {
    eprintln!(
        "Uso: __NAME__ [OPCIONES] [input]\n\
         \n\
         Opciones:\n\
         \x20 -h, --help     Muestra ayuda\n\
         \x20 -v, --verbose  Salida detallada\n\
         \n\
         Ejemplos:\n\
         \x20 __NAME__ hola"
    );
}

fn run(args: &[String]) -> Result<i32> {
    let mut verbose = false;
    let mut input: Option<&str> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => { print_help(); return Ok(0); }
            "-v" | "--verbose" => verbose = true,
            a => input = Some(a),
        }
        i += 1;
    }
    if verbose {
        eprintln!("[verbose] Iniciando");
    }
    if let Some(val) = input {
        println!("{}", val);
    } else {
        println!("Hello from __NAME__");
    }
    Ok(0)
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let code = run(&args)?;
    std::process::exit(code);
}
"#, name);

    let files = vec![
        AppFile { path: "Cargo.toml".into(), content: cargo_toml },
        AppFile { path: "src/main.rs".into(), content: main_rs },
    ];

    (name.to_string(), files, format!("App Rust CLI en {}/", name))
}

fn rust_web(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let cargo_toml = format!(
        r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.7"
tokio = {{ version = "1", features = ["full"] }}
serde = {{ version = "1", features = ["derive"] }}
"#,
        name
    );

    let main_rs = replace_name(r#"use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Serialize)]
struct Health {
    status: String,
}

async fn health() -> Json<Health> {
    Json(Health { status: "ok".into() })
}

async fn index() -> &'static str {
    "Hello from __NAME__"
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health));
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    println!("Server en http://localhost:8080");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
"#, name);

    let files = vec![
        AppFile { path: "Cargo.toml".into(), content: cargo_toml },
        AppFile { path: "src/main.rs".into(), content: main_rs },
    ];

    (name.to_string(), files, format!("App Rust Web (axum) en {}/", name))
}

// ─── Bash generators ───────────────────────────────────────────────

fn bash_cli(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let content = replace_name(r#"#!/bin/bash
set -euo pipefail

VERSION="1.0"
SCRIPT="__NAME__"

print_help() {
    cat <<HELP
Uso: ${SCRIPT} [OPCIONES] [input]

Opciones:
  -h, --help     Muestra esta ayuda
  -v, --version  Muestra la version
  --verbose      Salida detallada

Ejemplos:
  ${SCRIPT} hola
  ${SCRIPT} --verbose
HELP
}

verbose=false
input=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help) print_help; exit 0 ;;
        -v|--version) echo "${SCRIPT} v${VERSION}"; exit 0 ;;
        --verbose) verbose=true; shift ;;
        *) input="$1"; shift ;;
    esac
done

$verbose && echo "[verbose] Iniciando" >&2

if [[ -n "$input" ]]; then
    echo "$input"
else
    echo "Hello from __NAME__"
fi
"#, name);

    let files = vec![
        AppFile { path: format!("{}.sh", name), content },
    ];

    (name.to_string(), files, format!("Script Bash CLI en {}/", name))
}

fn bash_admin(name: &str, _desc: &str) -> (String, Vec<AppFile>, String) {
    let content = replace_name(r#"#!/bin/bash
set -euo pipefail

VERSION="1.0"
SCRIPT="__NAME__"
LOG_FILE=""

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log() {
    local level="$1"
    local msg="$2"
    echo -e "$(date '+%Y-%m-%d %H:%M:%S') [${level}] ${msg}"
    if [[ -n "$LOG_FILE" ]]; then
        echo "$(date '+%Y-%m-%d %H:%M:%S') [${level}] ${msg}" >> "$LOG_FILE"
    fi
}

info()  { log "INFO" "$1"; }
warn()  { log "WARN" "$1" >&2; }
error() { log "ERROR" "$1" >&2; }

confirm() {
    echo -e -n "${YELLOW}$1 [y/N]: ${NC}"
    read -r resp
    [[ "$resp" =~ ^[yYsS] ]]
}

run_cmd() {
    info "Ejecutando: $*"
    if [[ "$DRY_RUN" == "true" ]]; then
        info "[dry-run] omitido: $*"
        return 0
    fi
    "$@"
}

print_help() {
    cat <<HELP
Uso: ${SCRIPT} [OPCIONES] <comando>

Opciones:
  -h, --help       Muestra ayuda
  -n, --dry-run    Simular sin ejecutar
  -v, --verbose    Salida detallada
  -l, --log FILE   Archivo de log
  --version        Muestra version

Comandos:
  status   Ver estado del sistema
  backup   Realizar respaldo
  clean    Limpiar archivos temporales

Ejemplos:
  ${SCRIPT} status
  ${SCRIPT} --dry-run backup
  ${SCRIPT} --log /var/log/mi_script.log status
HELP
}

DRY_RUN=false
VERBOSE=false
COMMAND=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help) print_help; exit 0 ;;
        --version) echo "${SCRIPT} v${VERSION}"; exit 0 ;;
        -n|--dry-run) DRY_RUN=true; shift ;;
        -v|--verbose) VERBOSE=true; shift ;;
        -l|--log) LOG_FILE="$2"; shift 2 ;;
        *) COMMAND="$1"; shift ;;
    esac
done

info "Script: __NAME__ v${VERSION}"

case "$COMMAND" in
    status)
        info "Estado del sistema"
        echo "Host: $(hostname)"
        echo "Uptime: $(uptime -p)"
        echo "Memoria:"
        free -h
        ;;
    backup)
        if $DRY_RUN || confirm "Realizar respaldo"; then
            run_cmd tar -czf "backup_$(date '+%Y%m%d_%H%M%S').tar.gz" .
            info "Respaldo completado"
        else
            info "Respaldo cancelado"
            exit 1
        fi
        ;;
    clean)
        if $DRY_RUN || confirm "Eliminar archivos temporales"; then
            run_cmd rm -rf /tmp/*
            info "Limpieza completada"
        else
            info "Limpieza cancelada"
            exit 1
        fi
        ;;
    "")
        error "No se especifico comando"
        print_help
        exit 1
        ;;
    *)
        error "Comando desconocido: ${COMMAND}"
        exit 1
        ;;
esac

info "Completado."
"#, name);

    let files = vec![
        AppFile { path: format!("{}.sh", name), content },
    ];

    (name.to_string(), files, format!("Script Admin Bash en {}/", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_app_type_cli() {
        for desc in &["cli app", "terminal tool", "manejar argumentos", "subcomandos"] {
            assert_eq!(detect_app_type(desc), AppType::Cli, "fallo: {}", desc);
        }
    }

    #[test]
    fn test_detect_app_type_web() {
        for desc in &["web app", "api rest", "servidor http", "flask app", "express"] {
            assert_eq!(detect_app_type(desc), AppType::Web, "fallo: {}", desc);
        }
    }

    #[test]
    fn test_detect_app_type_desktop() {
        for desc in &["escritorio app", "gui interface", "ventana tkinter", "desktop tool", "interfaz grafica"] {
            assert_eq!(detect_app_type(desc), AppType::Desktop, "fallo: {}", desc);
        }
    }

    #[test]
    fn test_detect_app_type_admin() {
        for desc in &["admin script", "sistema backup", "servicio monitoreo", "respaldo automatico", "daemon"] {
            assert_eq!(detect_app_type(desc), AppType::Admin, "fallo: {}", desc);
        }
    }

    #[test]
    fn test_detect_app_type_doc() {
        for desc in &["doc report", "reporte html", "informe markdown", "generar pdf"] {
            assert_eq!(detect_app_type(desc), AppType::Doc, "fallo: {}", desc);
        }
    }

    #[test]
    fn test_detect_app_type_default() {
        assert_eq!(detect_app_type("hola mundo"), AppType::Cli);
    }

    #[test]
    fn test_normalize_language_python() {
        assert_eq!(normalize_language("python"), "python");
        assert_eq!(normalize_language("py"), "python");
    }

    #[test]
    fn test_normalize_language_node() {
        assert_eq!(normalize_language("node"), "node");
        assert_eq!(normalize_language("js"), "node");
        assert_eq!(normalize_language("javascript"), "node");
    }

    #[test]
    fn test_normalize_language_rust() {
        assert_eq!(normalize_language("rust"), "rust");
        assert_eq!(normalize_language("rs"), "rust");
    }

    #[test]
    fn test_normalize_language_bash() {
        assert_eq!(normalize_language("bash"), "bash");
        assert_eq!(normalize_language("sh"), "bash");
        assert_eq!(normalize_language("shell"), "bash");
    }

    #[test]
    fn test_replace_name() {
        assert_eq!(replace_name("hello __NAME__", "world"), "hello world");
    }

    #[test]
    fn test_generate_python_cli() {
        let t = generate_app("python", AppType::Cli, "test_app", "cli app");
        assert!(t.files.iter().any(|f| f.path.contains(".py")));
        assert!(t.files.iter().any(|f| f.path.contains("test")));
        assert!(t.files[0].content.contains("argparse"));
    }

    #[test]
    fn test_generate_python_desktop() {
        let t = generate_app("python", AppType::Desktop, "test_app", "desktop app");
        assert!(t.files[0].content.contains("tkinter"));
    }

    #[test]
    fn test_generate_python_web() {
        let t = generate_app("python", AppType::Web, "test_app", "web app");
        assert!(t.files[0].content.contains("http.server"));
        assert!(t.files[0].content.contains("do_GET"));
    }

    #[test]
    fn test_generate_python_admin() {
        let t = generate_app("python", AppType::Admin, "test_app", "admin");
        assert!(t.files[0].content.contains("dry_run"));
        assert!(t.files[0].content.contains("confirm"));
    }

    #[test]
    fn test_generate_python_doc() {
        let t = generate_app("python", AppType::Doc, "test_app", "report");
        assert!(t.files[0].content.contains("generate_html"));
        assert!(t.files[0].content.contains("generate_markdown"));
    }

    #[test]
    fn test_generate_node_cli() {
        let t = generate_app("node", AppType::Cli, "test_app", "cli");
        assert!(t.files.iter().any(|f| f.path.contains(".js")));
        assert!(t.files[0].content.contains("printHelp"));
    }

    #[test]
    fn test_generate_node_web() {
        let t = generate_app("node", AppType::Web, "test_app", "web");
        assert!(t.files[0].content.contains("http.createServer"));
    }

    #[test]
    fn test_generate_rust_cli() {
        let t = generate_app("rust", AppType::Cli, "test_app", "cli");
        assert!(t.files.iter().any(|f| f.path == "Cargo.toml"));
        assert!(t.files.iter().any(|f| f.path == "src/main.rs"));
        assert!(t.files[0].content.contains("anyhow"));
    }

    #[test]
    fn test_generate_rust_web() {
        let t = generate_app("rust", AppType::Web, "test_app", "web");
        assert!(t.files.iter().any(|f| f.path == "Cargo.toml"));
        assert!(t.files[1].content.contains("axum"));
    }

    #[test]
    fn test_generate_bash_cli() {
        let t = generate_app("bash", AppType::Cli, "test_app", "cli");
        assert!(t.files[0].content.contains("#!/bin/bash"));
        assert!(t.files[0].content.contains("print_help"));
    }

    #[test]
    fn test_generate_bash_admin() {
        let t = generate_app("bash", AppType::Admin, "test_app", "admin");
        assert!(t.files[0].content.contains("DRY_RUN"));
        assert!(t.files[0].content.contains("confirm"));
        assert!(t.files[0].content.contains("log"));
    }

    #[test]
    fn test_project_to_commands_creates_mkdir() {
        let t = generate_app("python", AppType::Cli, "test_app", "test");
        let cmds = project_to_commands(&t);
        assert!(cmds[0].1.contains("mkdir"));
        assert!(cmds[0].1.contains("cat >"));
    }

    #[test]
    fn test_project_to_commands_includes_files() {
        let t = generate_app("python", AppType::Cli, "test_app", "test");
        let cmds = project_to_commands(&t);
        for file in &t.files {
            let full = format!("{}/{}", t.dir, file.path);
            assert!(cmds[0].1.contains(&full), "falta: {}", full);
        }
    }

    #[test]
    fn test_generate_app_unknown_language_fallsback_to_python() {
        let t = generate_app("unknown", AppType::Cli, "test", "test");
        assert!(t.files[0].content.contains("argparse"));
    }

    #[test]
    fn test_generate_app_unknown_type_defaults_to_cli() {
        let t = generate_app("python", AppType::Cli, "test", "sume y reste");
        assert!(t.files[0].content.contains("argparse"));
    }

    #[test]
    fn test_generate_app_empty_name_uses_default() {
        let t = generate_app("python", AppType::Cli, "", "cli app");
        assert_eq!(t.dir, "app_python");
    }

    #[test]
    fn test_extension_for_languages() {
        assert_eq!(extension("python"), "py");
        assert_eq!(extension("node"), "js");
        assert_eq!(extension("rust"), "rs");
        assert_eq!(extension("bash"), "sh");
    }

    #[test]
    fn test_files_are_unique() {
        let t = generate_app("python", AppType::Cli, "test", "test");
        let mut paths: Vec<&str> = t.files.iter().map(|f| f.path.as_str()).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), t.files.len(), "hay archivos duplicados");
    }
}
