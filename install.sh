#!/bin/bash
set -euo pipefail

# ─── Fe Installer ───────────────────────────────────────────────
#   Builds the release binary and installs it to ~/.cargo/bin/
#   Also sets up the default config if none exists.
# ────────────────────────────────────────────────────────────────

VERSION="0.1.0"
BINARY_NAME="fe"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/fe"
BIN_DIR="${CARGO_HOME:-$HOME/.cargo}/bin"
SESSIONS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/fe/sessions"
CHECKPOINTS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/fe/checkpoints"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${GREEN}✔${NC} $1"; }
warn()  { echo -e "${YELLOW}⚠${NC} $1"; }
error() { echo -e "${RED}✖${NC} $1"; }
header(){ echo -e "\n${BLUE}══ $1 ══${NC}\n"; }

# ─── Pre-flight checks ─────────────────────────────────────────

header "Pre-flight"

if ! command -v cargo &>/dev/null; then
    error "Rust/Cargo no está instalado."
    echo "  Instálalo con: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi
info "Rust $(rustc --version) detectado"

# ─── Build ─────────────────────────────────────────────────────
header "Compilando $BINARY_NAME v$VERSION (release)"

cargo build --release --manifest-path="$(dirname "$0")/Cargo.toml"
info "Compilación exitosa"

# ─── Install binary ────────────────────────────────────────────
header "Instalando binario"

mkdir -p "$BIN_DIR"
cp "$(dirname "$0")/target/release/$BINARY_NAME" "$BIN_DIR/$BINARY_NAME"
chmod +x "$BIN_DIR/$BINARY_NAME"
info "Binario instalado en $BIN_DIR/$BINARY_NAME"

# ─── Setup config ──────────────────────────────────────────────
header "Configuración"

mkdir -p "$CONFIG_DIR"
mkdir -p "$SESSIONS_DIR"

CONFIG_FILE="$CONFIG_DIR/config.toml"
if [ ! -f "$CONFIG_FILE" ]; then
    cp "$(dirname "$0")/config.toml.example" "$CONFIG_FILE"
    info "Configuración creada en $CONFIG_FILE"
else
    warn "Configuración existente en $CONFIG_FILE (no se sobrescribe)"
fi

mkdir -p "$CHECKPOINTS_DIR"
info "Checkpoints: $CHECKPOINTS_DIR"

# Ensure sessions dir exists
info "Sesiones: $SESSIONS_DIR"

# ─── Verify ────────────────────────────────────────────────────
header "Verificación"

if command -v "$BINARY_NAME" &>/dev/null; then
    INSTALLED_PATH=$(command -v "$BINARY_NAME")
    info "Fe instalado en: $INSTALLED_PATH"
else
    warn "Binario no está en PATH. Agrega '$BIN_DIR' a tu PATH."
    echo "  export PATH=\"\$PATH:$BIN_DIR\"  >> ~/.bashrc"
fi

echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Fe v$VERSION instalado correctamente${NC}"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo ""
echo "  Prueba con:"
echo "    fe doctor"
echo "    fe 'hola mundo'"
echo "    fe --help"
echo ""
