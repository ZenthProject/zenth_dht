#!/usr/bin/env bash
# ============================================================
# sign_release.sh — Signe un binaire et génère manifest.json
#
# Usage :
#   ./scripts/sign_release.sh <version> <private_key.pem> <dist_dir>
#
# Exemple :
#   ./scripts/sign_release.sh 0.2.0 ed25519_private.pem ./dist
#
# Pré-requis : openssl >= 3.0 (Ed25519 support)
#
# Génère dans <dist_dir> :
#   manifest.json       (lu par le DHT)
#   *.deb / *.AppImage  (binaires déjà présents)
# ============================================================
set -euo pipefail

VERSION="${1:?Usage: $0 <version> <private_key.pem> <dist_dir>}"
PRIVKEY="${2:?Usage: $0 <version> <private_key.pem> <dist_dir>}"
DIST="${3:?Usage: $0 <version> <private_key.pem> <dist_dir>}"

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${GREEN}[sign]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC} $*"; }

[ -f "$PRIVKEY" ] || { echo "Clé privée introuvable : $PRIVKEY"; exit 1; }
[ -d "$DIST"    ] || { echo "Dossier dist introuvable : $DIST";  exit 1; }

sign_file() {
    local file="$1"
    local sha256
    sha256=$(sha256sum "$file" | awk '{print $1}')

    # Ed25519 : on signe le sha256 hex (pas le binaire entier)
    local sig
    sig=$(printf '%s' "$sha256" \
        | openssl pkeyutl -sign -inkey "$PRIVKEY" \
        | base64 -w0)

    local size
    size=$(wc -c < "$file")

    echo "{\"file\":\"$(basename "$file")\",\"sha256\":\"$sha256\",\"size\":$size,\"signature\":\"$sig\"}"
}

log "Signature des binaires v${VERSION}..."

MANIFEST="{\"version\":\"${VERSION}\",\"notes\":\"Release v${VERSION}\""

# Cherche les binaires dans dist/
DEB=$(find "$DIST" -name "*.deb" | head -1 || true)
APPIMAGE=$(find "$DIST" -name "*.AppImage" | head -1 || true)

if [ -n "$DEB" ]; then
    log "  → $(basename "$DEB")"
    ENTRY=$(sign_file "$DEB")
    MANIFEST="${MANIFEST},\"linux-x86_64-deb\":${ENTRY}"
else
    warn "Aucun .deb trouvé dans $DIST"
fi

if [ -n "$APPIMAGE" ]; then
    log "  → $(basename "$APPIMAGE")"
    ENTRY=$(sign_file "$APPIMAGE")
    MANIFEST="${MANIFEST},\"linux-x86_64-appimage\":${ENTRY}"
else
    warn "Aucun .AppImage trouvé dans $DIST"
fi

MANIFEST="${MANIFEST}}"

echo "$MANIFEST" | python3 -m json.tool > "$DIST/manifest.json"

log "manifest.json généré :"
cat "$DIST/manifest.json"
echo ""
log "Copie vers le serveur :"
echo "  scp $DIST/manifest.json $DIST/*.deb $DIST/*.AppImage user@serveur:/srv/zenth/updates/"
