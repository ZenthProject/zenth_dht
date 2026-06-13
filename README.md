# Zenth DHT

Serveur de distribution de clés et de messagerie post-quantique pour l'écosystème Zenth.

Construit avec **Axum**, **Diesel (PostgreSQL)**, **Protobuf** et **Dilithium**.

---

## Architecture

### Principe fondamental : zéro route

Le DHT expose **un unique point d'entrée** (`/`) qui accepte deux types de connexions :

| Méthode HTTP | Usage |
|---|---|
| `POST /` | Requêtes API — payload Protobuf (`DhtRequest`) |
| `GET /` | Upgrade WebSocket — push temps réel |

Toutes les fonctionnalités sont encodées dans le champ `method` (entier) du `DhtRequest`. Il n'y a aucune route REST classique.

```
Client
  │
  ├─ POST /  →  decode DhtRequest  →  dispatch method  →  encode DhtResponse
  └─ GET  /  →  WebSocket upgrade  →  authentification session  →  push notifications
```

### Modèle de sécurité

- **Zéro PII** : les utilisateurs sont identifiés par un `user_hash_id` (SHA-256 de leur identité). Aucun nom d'utilisateur ni email n'est stocké.
- **Authentification post-quantique** : toutes les requêtes sensibles sont signées avec **Dilithium2** (clé publique 2592 octets, signature 2420 octets).
- **Challenge-Response** : la connexion se fait en deux phases — le serveur émet un nonce, le client le signe, le serveur vérifie et émet un token de session.
- **X3DH** : l'établissement de session entre deux utilisateurs utilise des bundles de clés (identité, signed prekey, one-time prekeys, Kyber) sans que le serveur ne voie jamais le contenu des messages.

---

## Méthodes disponibles

| ID | Nom | État | Description |
|---|---|---|---|
| 0 | `METHOD_UNKNOWN` | — | Erreur de décodage |
| 1 | `REGISTER` | ✅ | Création de compte |
| 2 | `LOGIN` | ✅ | Authentification challenge-response Dilithium |
| 3 | `DELETE` | ✅ | Suppression de compte (cascade) |
| 4 | `BLOCK` | ❌ | Non implémenté |
| 5 | `GROUP` | ❌ | Non implémenté |
| 6 | `CONTACT` | ✅ | Envoi de demande d'ami |
| 7 | `REPORT` | ❌ | Non implémenté |
| 8 | `LOOKUP_USER` | ✅ | Recherche utilisateur par hash |
| 9 | `SEND_FRIEND_REQUEST` | ✅ | Alias de CONTACT |
| 10 | `FETCH_FRIEND_REQUESTS` | ✅ | Récupération des demandes reçues |
| 11 | `RESPOND_FRIEND_REQUEST` | ✅ | Accepter / rejeter une demande |
| 12 | `SEND_MESSAGE` | ✅ | Envoi d'un message chiffré |
| 13 | `FETCH_MESSAGES` | ✅ | Récupération des messages |
| 14 | `FETCH_FRIEND_RESPONSES` | ✅ | Réponses aux demandes envoyées |
| 15 | `UPLOAD_PREKEYS` | ✅ | Envoi de one-time prekeys |
| 16 | `FETCH_PREKEY_BUNDLE` | ✅ | Bundle X3DH d'un utilisateur |
| 17 | `CHECK_PREKEY_COUNT` | ✅ | Nombre de prekeys disponibles |
| 18 | `REPLENISH_PREKEYS` | ✅ | Rechargement de prekeys |
| 19 | `GET_UPDATE_MANIFEST` | ✅ | Manifest de mise à jour client |
| 20 | `GET_UPDATE_CHUNK` | ✅ | Téléchargement du binaire par chunks |

---

## Schéma de base de données

### `users`
| Colonne | Type | Description |
|---|---|---|
| `user_hash_id` | `BYTEA PK` | SHA-256 de l'identité (32 octets) |
| `password_commitment` | `BYTEA` | Engagement AuCPACE |
| `identity_key_dilithium` | `BYTEA` | Clé publique Dilithium2 |
| `identity_signature` | `BYTEA` | Preuve de possession de la clé |
| `pre_key_bundle` | `BYTEA` | Bundle X3DH encodé Protobuf |
| `proof_type` | `INT4` | Type de preuve d'authentification |

### `auth_challenges`
Challenges de connexion expirés après 5 minutes. Nonce aléatoire 32 octets.

### `sessions`
Tokens de session 32 octets, durée de vie 24h.

### `friend_requests`
Demandes d'ami avec bundle X3DH de l'initiateur et signature Dilithium.
Contrainte unique `(requester_hash_id, target_hash_id)`.

### `friend_responses`
Réponses (acceptation/rejet) avec bundle X3DH si accepté. Tracking de livraison.

### `messages`
Messages chiffrés end-to-end. Le serveur ne stocke que des octets opaques.
Signature Dilithium sur `sender || recipient || message_id || timestamp || content`.

### `one_time_prekeys`
Prekeys à usage unique pour X3DH.
Index partiel sur `(user_hash_id) WHERE NOT used` pour les requêtes efficaces.

---

## WebSocket

### Authentification

Premier message du client après connexion :
```
[ user_hash_id (32 octets) | session_token (N octets) ]
```
Le serveur vérifie le token en base (non expiré). Déconnexion immédiate si invalide.

### Notifications push

Le serveur émet des `WsNotification` (Protobuf binaire) pour :
- Nouvelle demande d'ami entrante
- Réponse à une demande d'ami
- Nouveau message

### Requêtes via WebSocket

Les frames binaires reçues sont traitées comme des `DhtRequest` normaux — même dispatcher que HTTP POST.

---

## Mise à jour client

### Flux

```
CI/CD (zenth_front)
  → build : .deb, .AppImage, .exe, .apk
  → sign_release.sh : sha256 + signature Ed25519 → manifest.json
  → upload manifest.json + binaires vers RustFS (bucket zenth-release)

Client Tauri (toutes les 10 min)
  → METHOD 19 → DHT récupère manifest.json depuis RustFS → retourne version + sha256 + signature
  → si version > version courante → banner "Mise à jour disponible"
  → METHOD 20 (x N) → DHT streame le binaire depuis RustFS par chunks (max 512 Ko, HTTP Range)
  → client : vérifie SHA-256 + signature Ed25519 → installe selon le format détecté
              AppImage → copie en place + chmod 755
              .deb     → pkexec dpkg -i
              .exe     → lance l'installeur NSIS
              .apk     → opener Android (system package installer)
```

### Format manifest.json (RustFS)

```json
{
  "version": "0.2.0",
  "notes": "Description des changements",
  "linux-x86_64-appimage": {
    "file": "Zenth_0.2.0_linux.AppImage",
    "sha256": "a3f1b2c4...",
    "size": 15728640,
    "signature": "base64_ed25519_sig_sur_sha256"
  },
  "linux-x86_64-deb": {
    "file": "Zenth_0.2.0_amd64.deb",
    "sha256": "b4c5d6e7...",
    "size": 12582912,
    "signature": "base64_ed25519_sig_sur_sha256"
  },
  "windows-x86_64": {
    "file": "Zenth_0.2.0_x64.exe",
    "sha256": "c5d6e7f8...",
    "size": 9000000,
    "signature": "base64_ed25519_sig_sur_sha256"
  },
  "android-arm64": {
    "file": "Zenth_0.2.0_arm64.apk",
    "sha256": "d6e7f8a9...",
    "size": 8000000,
    "signature": "base64_ed25519_sig_sur_sha256"
  }
}
```

La signature Ed25519 couvre les **bytes du sha256 hex** (pas le binaire entier).
Les fichiers sont déposés à plat dans le bucket : `{RUSTFS_BASE_URL}/{file}`.

### Génération de la paire de clés (une seule fois)

```bash
./scripts/gen_update_key.sh
# → ed25519_private.pem  : stocker dans GitLab CI (ED25519_PRIVATE_KEY, base64)
# → ed25519_public.bin   : 32 bytes bruts à coller dans UPDATE_PUBKEY (client Tauri)
```

### Signer une release

```bash
./scripts/sign_release.sh 0.2.0 ed25519_private.pem ./dist
# → dist/manifest.json
```

### Changer la version minimale requise

La version minimale est stockée en base de données (`app_config`).
**Aucun redémarrage du DHT n'est nécessaire** — la valeur est lue à chaque login.

```bash
# Avec le script (charge DATABASE_URL depuis .env.prod)
./scripts/set_version.sh 0.2.0

# Ou directement en SQL
psql "$DATABASE_URL" -c "UPDATE app_config SET value = '0.2.0' WHERE key = 'required_version';"
```

Les clients avec une version inférieure reçoivent un `VersionOutdated` au login et voient le banner de mise à jour.

---

## TLS

Le serveur supporte **TLS 1.3 natif** (rustls) si des certificats sont présents :

```bash
TLS_CERT_PATH=certs/cert.pem
TLS_KEY_PATH=certs/key.pem
```

Sans certificats → plain HTTP/WS (dev ou derrière reverse proxy).

**En production avec Nginx Proxy Manager :**

| Proxy Host | Target | Options requises |
|---|---|---|
| `dht.zenth-project.com` | `http://127.0.0.1:3000` | SSL + Force HTTPS |
| `ws.zenth-project.com` | `http://127.0.0.1:3000` | SSL + **Websockets Support** |

---

## Variables d'environnement

| Variable | Obligatoire | Défaut | Description |
|---|---|---|---|
| `DATABASE_URL` | ✅ | — | URL PostgreSQL |
| `HOST` | ✅ | — | Adresse d'écoute (`0.0.0.0`) |
| `PORT` | ✅ | — | Port |
| `RUSTFS_BASE_URL` | ✅ | — | URL de base du bucket RustFS (`https://rustfs.example.com/zenth-release`) |
| `TLS_CERT_PATH` | ❌ | `certs/cert.pem` | Certificat TLS (absent = plain HTTP) |
| `TLS_KEY_PATH` | ❌ | `certs/key.pem` | Clé privée TLS |
| `RUST_LOG` | ❌ | `info` | Niveau de log |
| `PGSSLMODE` | ❌ | `require` | SSL PostgreSQL (`disable` en dev) |

> La version minimale requise n'est **pas** une variable d'environnement.
> Elle est stockée en base (`app_config`) et modifiable à chaud via `./scripts/set_version.sh`.

---

## Développement

### Pré-requis

- Rust stable
- Docker + Docker Compose
- `diesel_cli` : `cargo install diesel_cli --no-default-features --features postgres`

### Lancer en local

```bash
# Lancer PostgreSQL + migrations + app
docker compose up --build

# Ou PostgreSQL uniquement, app en cargo run
docker compose up postgres migrations
cargo run
# (variables lues depuis .env)
```

Pour simuler RustFS en local (test du système de mise à jour) :
```bash
mkdir -p /tmp/faux-rustfs
# → déposer manifest.json + binaires dans /tmp/faux-rustfs/
python3 -m http.server 9000 --directory /tmp/faux-rustfs
# puis RUSTFS_BASE_URL=http://localhost:9000 dans .env
```

### Migrations

```bash
diesel migration run        # Appliquer
diesel migration revert     # Revenir en arrière
diesel migration generate <nom>   # Nouvelle migration
```

---

## Production

### Build des images Docker

```bash
export GITLAB_TOKEN=<token_read_repository>

make all          # Build app + migrations + push
make release      # Build image app uniquement
make migrations   # Build image diesel CLI
make push         # Push vers le registre
```

Le `GITLAB_TOKEN` est passé via **BuildKit secret** — jamais intégré dans l'image.

### Déployer

```bash
cp .env.prod.example .env.prod
# → remplir POSTGRES_PASSWORD, REGISTRY, APP_VERSION, TAG

./deploy.sh              # pull + migrate + restart
./deploy.sh --build      # build local + deploy
./deploy.sh --rollback   # revenir à l'image précédente
```

### Variables CI/CD GitLab

| Variable | Description |
|---|---|
| `GITLAB_TOKEN` | Accès aux dépôts privés (build Docker) |
| `ED25519_PRIVATE_KEY` | Clé de signature des releases (base64) |
| `DEPLOY_SSH_KEY` | Clé SSH pour SCP vers le serveur (base64) |
| `DEPLOY_HOST` | Adresse du serveur |
| `DEPLOY_USER` | Utilisateur SSH |

---

## Kubernetes

```bash
kubectl create secret generic zenth-dht-secrets \
  --from-literal=database-url='postgres://user:pass@host/db?sslmode=require'

kubectl apply -f k8s-deployment.yaml
```

Manifeste inclus : 2 réplicas, init container migrations, resource limits, SeccompDefault.

---

## Structure du projet

```
zenth_dht/
├── src/
│   ├── main.rs                    # Point d'entrée, TLS, routing
│   ├── db.rs                      # Connexion PostgreSQL
│   ├── crypto.rs                  # Vérification Dilithium
│   ├── models.rs                  # Modèles Diesel
│   ├── schema.rs                  # Schéma généré
│   ├── timestamp.rs               # Timestamp Unix
│   ├── errors/error404.rs         # Fallback 404
│   ├── handlers/
│   │   ├── decompose.rs           # Dispatcher central
│   │   └── method/                # Un fichier par méthode (1–20)
│   └── websocket/
│       ├── connection_manager.rs  # Broadcast par user_hash
│       └── handler.rs             # Auth WS + dispatch
├── migrations/                    # Diesel migrations SQL
├── scripts/
│   ├── gen_update_key.sh          # Génère la paire Ed25519
│   └── sign_release.sh            # Signe les binaires → manifest.json
├── Dockerfile                     # Multi-stage, Alpine runtime
├── Dockerfile.migrations          # Image diesel CLI
├── docker-compose.yml             # Dev
├── docker-compose.prod.yml        # Prod
├── deploy.sh                      # Script de déploiement
├── Makefile                       # Build + push Docker
└── k8s-deployment.yaml            # Kubernetes
```

---

## Dépendances principales

| Crate | Usage |
|---|---|
| `axum 0.8` | Framework HTTP/WebSocket |
| `tokio 1` | Runtime async |
| `diesel 2` | ORM PostgreSQL |
| `prost 0.14` | Sérialisation Protobuf |
| `rustls 0.23` | TLS 1.3 |
| `pqcrypto-dilithium 0.5` | Signatures post-quantiques Dilithium2 |
| `pqcrypto-kyber 0.8` | KEM post-quantique Kyber |
| `aucpace 0.1` | Authentification par mot de passe (PAKE) |
| `zenth_dto` | Types Protobuf partagés |
| `zenth_crypto` | Primitives cryptographiques Zenth |
