#!/bin/bash
set -euo pipefail

# ─── Fe Release Packager ────────────────────────────────────────
#   Crea fe-v0.1.0.tar.gz con binario + docs + checksums
# ────────────────────────────────────────────────────────────────

VERSION="0.1.0"
NAME="fe"
RELEASE_DIR="target/release"
PACKAGE_NAME="${NAME}-v${VERSION}"
PACKAGE_DIR="/tmp/${PACKAGE_NAME}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info()  { echo -e "${GREEN}✔${NC} $1"; }
warn()  { echo -e "${YELLOW}⚠${NC} $1"; }
header(){ echo -e "\n${BLUE}══ $1 ══${NC}\n"; }

PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ─── Ensure release build exists ───────────────────────────────

header "Release v${VERSION}"

if [ ! -f "${PROJECT_DIR}/${RELEASE_DIR}/${NAME}" ]; then
    info "Compilando release..."
    (cd "$PROJECT_DIR" && cargo build --release)
else
    info "Release build encontrado"
fi

BINARY="${PROJECT_DIR}/${RELEASE_DIR}/${NAME}"
BINARY_SIZE=$(stat -c%s "$BINARY" 2>/dev/null || stat -f%z "$BINARY" 2>/dev/null)
BINARY_SIZE_MB=$((BINARY_SIZE / 1048576))
info "Binario: ${BINARY_SIZE_MB}MB"

# ─── Create package directory ──────────────────────────────────

header "Empaquetando"

rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR"

# Binary
cp "$BINARY" "$PACKAGE_DIR/${NAME}"
chmod +x "$PACKAGE_DIR/${NAME}"
info "Binario copiado"

# Docs
mkdir -p "$PACKAGE_DIR/docs"
cp "${PROJECT_DIR}/README.md" "$PACKAGE_DIR/"
cp "${PROJECT_DIR}/CHANGELOG.md" "$PACKAGE_DIR/"
cp "${PROJECT_DIR}/ARCHITECTURE.md" "$PACKAGE_DIR/"
cp "${PROJECT_DIR}/config.toml.example" "$PACKAGE_DIR/"
cp "${PROJECT_DIR}/docs/user-guide.md" "$PACKAGE_DIR/docs/"
cp "${PROJECT_DIR}/docs/configuration.md" "$PACKAGE_DIR/docs/"
info "Documentación copiada"

# Install script
cp "${PROJECT_DIR}/install.sh" "$PACKAGE_DIR/"
chmod +x "$PACKAGE_DIR/install.sh"
info "Script instalador copiado"

# ─── Create tar.gz ─────────────────────────────────────────────

TARBALL="${PROJECT_DIR}/${PACKAGE_NAME}.tar.gz"

cd /tmp
tar -czf "$TARBALL" "$PACKAGE_NAME"
cd "$PROJECT_DIR"

info "Package: ${TARBALL}"

# ─── Generate checksums ────────────────────────────────────────

header "Checksums"

CHECKSUM_FILE="${PROJECT_DIR}/${PACKAGE_NAME}.sha256"
cd "${PROJECT_DIR}"

sha256sum "${PACKAGE_NAME}.tar.gz" > "$CHECKSUM_FILE"
info "SHA256: $(cut -d' ' -f1 "$CHECKSUM_FILE")"

# Generate per-file checksums
cd /tmp
find "$PACKAGE_NAME" -type f -exec sha256sum {} \; > "${PROJECT_DIR}/${PACKAGE_NAME}.files.sha256"
cd "$PROJECT_DIR"
info "File checksums: ${PACKAGE_NAME}.files.sha256"

# ─── Summary ───────────────────────────────────────────────────

header "Release v${VERSION} lista"

echo "  Package:    ${TARBALL}"
echo "  Size:       $(du -h "$TARBALL" | cut -f1)"
echo "  SHA256:     $(cut -d' ' -f1 "$CHECKSUM_FILE")"
echo "  Contenido:"
tar -tzf "$TARBALL" | sed 's/^/    /'
echo ""
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${GREEN}  Para publicar:${NC}"
echo "    git tag v${VERSION}"
echo "    git push --tags"
echo "    gh release create v${VERSION} ${PACKAGE_NAME}.tar.gz ${PACKAGE_NAME}.sha256 ${PACKAGE_NAME}.files.sha256"
echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
