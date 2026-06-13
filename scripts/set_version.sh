#!/usr/bin/env bash
# Modifie la version minimale requise pour se connecter au DHT.
# Les clients avec une version inférieure reçoivent un VersionOutdated au login.
# Aucun redémarrage du DHT requis — la valeur est lue en base à chaque connexion.
#
# Usage:
#   ./scripts/set_version.sh 0.1.1
#   DATABASE_URL=postgres://... ./scripts/set_version.sh 0.1.1

set -euo pipefail

VERSION="${1:-}"
if [[ -z "$VERSION" ]]; then
    echo "Usage: $0 <version>"
    echo "Exemple: $0 0.1.1"
    exit 1
fi

# Charge .env.prod si présent
if [[ -f .env.prod ]]; then
    # shellcheck disable=SC1091
    set -a && source .env.prod && set +a
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
    echo "Erreur : DATABASE_URL non définie."
    echo "Définissez-la dans .env.prod ou en variable d'environnement."
    exit 1
fi

CURRENT=$(psql "$DATABASE_URL" -t -c "SELECT value FROM app_config WHERE key = 'required_version';" | tr -d ' ')

echo "Version actuelle : ${CURRENT:-<non définie>}"
echo "Nouvelle version : $VERSION"
echo ""

psql "$DATABASE_URL" -c "UPDATE app_config SET value = '$VERSION' WHERE key = 'required_version';"

echo "Fait. Les clients < $VERSION seront bloqués au prochain login."
