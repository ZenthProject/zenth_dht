#!/usr/bin/env bash
# ============================================================
# deploy.sh — Déploiement Zenth DHT en production
#
# Usage :
#   ./deploy.sh              → déploie avec .env.prod
#   ./deploy.sh --build      → build les images localement avant
#   ./deploy.sh --rollback   → revient à l'image précédente
#
# Pré-requis : docker, docker compose v2, GITLAB_TOKEN dans l'env
#              (uniquement pour --build)
# ============================================================
set -euo pipefail

# ── Config ────────────────────────────────────────────────────
COMPOSE_FILE="docker-compose.prod.yml"
ENV_FILE=".env.prod"
APP_SERVICE="app"

# ── Couleurs ──────────────────────────────────────────────────
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

log()   { echo -e "${GREEN}[deploy]${NC} $*"; }
info()  { echo -e "${BLUE}[info]${NC}   $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC}   $*"; }
error() { echo -e "${RED}[error]${NC}  $*" >&2; exit 1; }

# ── Vérifications initiales ───────────────────────────────────
[ -f "$ENV_FILE" ] || error "Fichier $ENV_FILE introuvable. Copie .env.prod.example et remplis-le."
docker info > /dev/null 2>&1 || error "Docker n'est pas lancé."
docker compose version > /dev/null 2>&1 || error "docker compose v2 requis."

COMPOSE="docker compose --env-file $ENV_FILE -f $COMPOSE_FILE"

# ── Fonctions ─────────────────────────────────────────────────

build_images() {
    log "Build des images localement..."
    [ -n "${GITLAB_TOKEN:-}" ] || error "GITLAB_TOKEN non défini (requis pour accéder aux crates privées)."

    DOCKER_BUILDKIT=1 docker build \
        --secret id=gitlab_token,env=GITLAB_TOKEN \
        -f Dockerfile \
        -t "$(grep REGISTRY $ENV_FILE | cut -d= -f2)/$(grep APP_NAME $ENV_FILE | cut -d= -f2):latest" \
        .

    DOCKER_BUILDKIT=1 docker build \
        -f Dockerfile.migrations \
        -t "$(grep REGISTRY $ENV_FILE | cut -d= -f2)/$(grep APP_NAME $ENV_FILE | cut -d= -f2)-migrations:latest" \
        .

    log "Images buildées."
}

pull_images() {
    log "Pull des dernières images..."
    $COMPOSE pull --quiet
    log "Images à jour."
}

save_previous_tag() {
    # Sauvegarde le digest de l'image actuelle pour rollback
    PREV=$($COMPOSE images -q "$APP_SERVICE" 2>/dev/null || true)
    if [ -n "$PREV" ]; then
        echo "$PREV" > .last_image_id
        info "Image précédente sauvegardée : $PREV"
    fi
}

run_migrations() {
    log "Application des migrations..."
    $COMPOSE run --rm migrations
    log "Migrations OK."
}

start_services() {
    log "Démarrage des services..."
    $COMPOSE up -d --remove-orphans postgres
    log "Attente que PostgreSQL soit prêt..."
    $COMPOSE up --no-deps -d --wait postgres 2>/dev/null || true

    run_migrations

    log "Démarrage de l'application..."
    $COMPOSE up -d --no-deps --remove-orphans "$APP_SERVICE"
}

check_health() {
    log "Vérification que l'app répond (max 60s)..."
    local waited=0
    local max=60

    while [ $waited -lt $max ]; do
        if $COMPOSE ps "$APP_SERVICE" | grep -q "Up"; then
            # Vérifie que le port répond
            if bash -c "exec 3<>/dev/tcp/127.0.0.1/3000" 2>/dev/null; then
                exec 3>&- 2>/dev/null || true
                log "App opérationnelle sur 127.0.0.1:3000"
                return 0
            fi
        fi
        waited=$((waited + 5))
        sleep 5
        echo -n "."
    done

    echo ""
    error "L'app ne répond pas après ${max}s. Lance './deploy.sh --rollback' si nécessaire."
}

rollback() {
    warn "Rollback vers l'image précédente..."
    if [ ! -f .last_image_id ]; then
        error "Pas d'image précédente sauvegardée."
    fi

    local prev
    prev=$(cat .last_image_id)

    # Source le registry/app_name depuis le .env.prod
    local registry app_name
    registry=$(grep ^REGISTRY "$ENV_FILE" | cut -d= -f2)
    app_name=$(grep ^APP_NAME  "$ENV_FILE" | cut -d= -f2)

    docker tag "$prev" "${registry}/${app_name}:latest"
    $COMPOSE up -d --no-deps "$APP_SERVICE"
    warn "Rollback effectué. Vérifie les logs : $COMPOSE logs -f $APP_SERVICE"
}

show_status() {
    echo ""
    log "État des services :"
    $COMPOSE ps
    echo ""
    info "Logs récents :"
    $COMPOSE logs --tail=20 "$APP_SERVICE"
}

# ── Point d'entrée ────────────────────────────────────────────
case "${1:-}" in
    --build)
        build_images
        save_previous_tag
        run_migrations
        start_services
        check_health
        show_status
        ;;
    --rollback)
        rollback
        ;;
    "")
        pull_images
        save_previous_tag
        run_migrations
        start_services
        check_health
        show_status
        ;;
    *)
        echo "Usage: $0 [--build | --rollback]"
        exit 1
        ;;
esac

log "Déploiement terminé."
echo ""
echo -e "${BLUE}NPM : configure ces deux proxy hosts → http://127.0.0.1:3000${NC}"
echo -e "  - dht.zenth-project.com  (SSL Let's Encrypt)"
echo -e "  - ws.zenth-project.com   (SSL Let's Encrypt + Websockets Support)"
