#!/usr/bin/env bash
# ============================================================
# gen_update_key.sh — Génère la paire de clés Ed25519 pour
# signer les mises à jour.
#
# Usage : ./scripts/gen_update_key.sh
#
# Produit :
#   ed25519_private.pem  → à stocker dans GitLab CI (variable ED25519_PRIVATE_KEY, base64)
#   ed25519_public.bin   → 32 bytes bruts à coller dans UPDATE_PUBKEY de pages/update/mod.rs
# ============================================================
set -euo pipefail

GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
log()  { echo -e "${GREEN}[keygen]${NC} $*"; }
warn() { echo -e "${YELLOW}[warn]${NC}  $*"; }

log "Génération de la paire de clés Ed25519..."
openssl genpkey -algorithm ED25519 -out ed25519_private.pem
openssl pkey -in ed25519_private.pem -pubout -out ed25519_public.pem

# Extrait les 32 bytes bruts de la clé publique (retire l'en-tête DER)
openssl pkey -in ed25519_private.pem -pubout -outform DER \
    | tail -c 32 > ed25519_public.bin

# Affiche le tableau Rust à copier dans pages/update/mod.rs
log "Clé publique (à coller dans UPDATE_PUBKEY dans pages/update/mod.rs) :"
echo ""
python3 - <<'EOF'
with open("ed25519_public.bin", "rb") as f:
    data = f.read()
hex_bytes = ", ".join(f"0x{b:02x}" for b in data)
print(f"const UPDATE_PUBKEY: &[u8; 32] = &[")
# 8 par ligne
chunks = [data[i:i+8] for i in range(0, len(data), 8)]
for chunk in chunks:
    print("    " + ", ".join(f"0x{b:02x}" for b in chunk) + ",")
print("];")
EOF

echo ""
warn "Stocke la clé privée dans GitLab CI :"
echo "  Settings → CI/CD → Variables → ED25519_PRIVATE_KEY"
echo "  Valeur : \$(base64 -w0 ed25519_private.pem)"
echo ""
warn "Ne committe JAMAIS ed25519_private.pem dans git."
echo "  Ajoute-le à .gitignore : echo 'ed25519_private.pem' >> .gitignore"
