# ============================================================
# Makefile — build Docker (compilation Rust dans le container)
# Usage : make release      (build image app)
#         make migrations   (build image migrations)
#         make push         (push les deux images)
#         make all          (tout d'un coup)
#
# Pré-requis pour `make release` :
#   export GITLAB_TOKEN=<ton token GitLab>
# ============================================================

APP_NAME     := zenth_dht
REGISTRY     := ghcr.io/ton-org   # ← adapte ici
TAG          := $(shell git rev-parse --short HEAD)
IMG_APP      := $(REGISTRY)/$(APP_NAME):$(TAG)
IMG_APP_LAST := $(REGISTRY)/$(APP_NAME):latest
IMG_MIG      := $(REGISTRY)/$(APP_NAME)-migrations:$(TAG)
IMG_MIG_LAST := $(REGISTRY)/$(APP_NAME)-migrations:latest

.PHONY: all release migrations push push-app push-migrations clean

all: release migrations push

## 1. Build image app — Rust compilé dans Docker via BuildKit secret
release:
	DOCKER_BUILDKIT=1 docker build \
		--file Dockerfile \
		--secret id=gitlab_token,env=GITLAB_TOKEN \
		--tag $(IMG_APP) \
		--tag $(IMG_APP_LAST) \
		.

## 2. Build image migrations — diesel_cli compilé dans Docker
migrations:
	DOCKER_BUILDKIT=1 docker build \
		--file Dockerfile.migrations \
		--tag $(IMG_MIG) \
		--tag $(IMG_MIG_LAST) \
		.

## 3. Push tout
push: push-app push-migrations

push-app:
	docker push $(IMG_APP)
	docker push $(IMG_APP_LAST)

push-migrations:
	docker push $(IMG_MIG)
	docker push $(IMG_MIG_LAST)

clean:
	docker rmi $(IMG_APP) $(IMG_APP_LAST) $(IMG_MIG) $(IMG_MIG_LAST) || true
