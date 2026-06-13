#!/usr/bin/env bash
# ============================================================
# deploy-bin.sh — Déploiement du binaire zenth_dht (sans Docker)
#
# Usage :
#   ./deploy-bin.sh                    → déploie le binaire local
#   ./deploy-bin.sh --rollback         → revient au binaire précédent
#   ./deploy-bin.sh --status           → affiche l'état du service
#
# Pré-requis :
#   - binaire compilé dans ./target/release/zenth_dht
#   - PostgreSQL accessible sur localhost:5432
#   - systemd disponible
#   - sudo disponible
# ============================================================
set -euo pipefail

# ── Config ────────────────────────────────────────────────────
BIN_SRC="./target/release/zenth_dht"
BIN_DEST="/srv/docker/d-builds/zenth_dht/zenth_dht"
BIN_BACKUP="${BIN_DEST}.prev"
SERVICE_NAME="zenth-dht"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"
ENV_FILE=".env.prod"
PORT=8081

# ── Couleurs ──────────────────────────────────────────────────
GREEN='\033[0;32m'; YELLOW='\033[1;33m'; RED='\033[0;31m'; BLUE='\033[0;34m'; NC='\033[0m'
log()   { echo -e "${GREEN}[deploy]${NC} $*"; }
info()  { echo -e "${BLUE}[info]${NC}   $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC}   $*"; }
error() { echo -e "${RED}[error]${NC}  $*" >&2; exit 1; }

# ── Charger les variables d'env ───────────────────────────────
load_env() {
    [ -f "$ENV_FILE" ] || error "Fichier $ENV_FILE introuvable."
    set -a; source "$ENV_FILE"; set +a
}

# ── Vérifications ─────────────────────────────────────────────
check_prereqs() {
    [ -f "$BIN_SRC" ] || error "Binaire introuvable : $BIN_SRC — lance 'cargo build --release' d'abord."
    command -v systemctl > /dev/null 2>&1 || error "systemd requis."
    # Vérifie que PostgreSQL répond
    if command -v pg_isready > /dev/null 2>&1; then
        pg_isready -h localhost -p 5432 -U "$POSTGRES_USER" > /dev/null 2>&1 \
            || warn "PostgreSQL ne répond pas encore sur localhost:5432 — continue quand même."
    fi
}

# ── Installer le service systemd ──────────────────────────────
install_service() {
    if [ -f "$SERVICE_FILE" ]; then
        info "Service systemd déjà présent, on le met à jour."
        return
    fi

    log "Création du service systemd ${SERVICE_NAME}..."
    sudo tee "$SERVICE_FILE" > /dev/null <<EOF
[Unit]
Description=Zenth DHT
After=network.target

[Service]
Type=simple
ExecStart=${BIN_DEST}
WorkingDirectory=$(dirname "$BIN_DEST")
User=root
Restart=on-failure
RestartSec=5s

# Variables d'environnement
Environment=DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:5432/${POSTGRES_DB}?sslmode=disable
Environment=RUST_LOG=${RUST_LOG:-info}
Environment=HOST=127.0.0.1
Environment=PORT=${PORT}
Environment=APP_VERSION=${APP_VERSION:-0.1.0}
Environment=PGSSLMODE=disable
Environment=UPDATES_DIR=/srv/zenth/updates

# Sécurité minimale
NoNewPrivileges=yes
ProtectSystem=full

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable "$SERVICE_NAME"
    log "Service ${SERVICE_NAME} installé et activé."
}

# ── Mettre à jour le service avec les nouvelles env vars ──────
update_service_env() {
    sudo sed -i \
        -e "s|Environment=DATABASE_URL=.*|Environment=DATABASE_URL=postgres://${POSTGRES_USER}:${POSTGRES_PASSWORD}@localhost:5432/${POSTGRES_DB}?sslmode=disable|" \
        -e "s|Environment=RUST_LOG=.*|Environment=RUST_LOG=${RUST_LOG:-info}|" \
        -e "s|Environment=APP_VERSION=.*|Environment=APP_VERSION=${APP_VERSION:-0.1.0}|" \
        "$SERVICE_FILE"
    sudo systemctl daemon-reload
}

# ── Sauvegarder et déployer le binaire ────────────────────────
deploy_binary() {
    # Sauvegarde l'ancien binaire pour rollback
    if [ -f "$BIN_DEST" ]; then
        cp "$BIN_DEST" "$BIN_BACKUP"
        info "Ancien binaire sauvegardé → ${BIN_BACKUP}"
    fi

    log "Copie du binaire vers ${BIN_DEST}..."
    cp "$BIN_SRC" "$BIN_DEST"
    chmod 755 "$BIN_DEST"
    log "Binaire déployé."
}

# ── Redémarrer le service ─────────────────────────────────────
restart_service() {
    log "Redémarrage de ${SERVICE_NAME}..."
    if sudo systemctl is-active --quiet "$SERVICE_NAME"; then
        sudo systemctl restart "$SERVICE_NAME"
    else
        sudo systemctl start "$SERVICE_NAME"
    fi
}

# ── Vérifier que l'app répond ─────────────────────────────────
check_health() {
    log "Vérification que l'app répond sur 127.0.0.1:${PORT} (max 30s)..."
    local waited=0
    while [ $waited -lt 30 ]; do
        if bash -c "exec 3<>/dev/tcp/127.0.0.1/${PORT}" 2>/dev/null; then
            exec 3>&- 2>/dev/null || true
            log "App opérationnelle sur 127.0.0.1:${PORT} ✓"
            return 0
        fi
        waited=$((waited + 2))
        sleep 2
        echo -n "."
    done
    echo ""
    warn "L'app ne répond pas après 30s."
    warn "Vérifie les logs : sudo journalctl -u ${SERVICE_NAME} -n 50"
    warn "Ou rollback      : ./deploy-bin.sh --rollback"
    return 1
}

# ── Rollback ─────────────────────────────────────────────────
rollback() {
    warn "Rollback vers le binaire précédent..."
    [ -f "$BIN_BACKUP" ] || error "Pas de binaire précédent trouvé (${BIN_BACKUP} absent)."
    cp "$BIN_BACKUP" "$BIN_DEST"
    chmod 755 "$BIN_DEST"
    restart_service
    warn "Rollback effectué."
    sudo journalctl -u "$SERVICE_NAME" -n 20 --no-pager
}

# ── Status ────────────────────────────────────────────────────
show_status() {
    echo ""
    log "État du service :"
    sudo systemctl status "$SERVICE_NAME" --no-pager -l || true
    echo ""
    info "Logs récents :"
    sudo journalctl -u "$SERVICE_NAME" -n 20 --no-pager || true
}

# ── Point d'entrée ────────────────────────────────────────────
case "${1:-}" in
    --rollback)
        load_env
        rollback
        ;;
    --status)
        show_status
        ;;
    "")
        load_env
        check_prereqs
        install_service
        update_service_env
        deploy_binary
        restart_service
        check_health
        show_status

        echo ""
        echo -e "${BLUE}NPM : configure ces deux proxy hosts → http://127.0.0.1:${PORT}${NC}"
        echo -e "  - dht.zenth-project.com  (SSL Let's Encrypt)"
        echo -e "  - ws.zenth-project.com   (SSL Let's Encrypt + Websockets Support)"
        echo ""
        log "Déploiement terminé."
        ;;
    *)
        echo "Usage: $0 [--rollback | --status]"
        exit 1
        ;;
esac
