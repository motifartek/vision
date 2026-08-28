SHELL := /bin/sh

.PHONY: help run\:dev dashboard gateway identity stream infra\:up infra\:down infra\:logs admin

.DEFAULT_GOAL := help

# Provide harmless placeholders for service selectors so make does not error
# when a user passes an application name as an additional argument.
dashboard gateway identity stream:
	@:

# Supported development services for the repository.
DEV_APPS := dashboard gateway identity stream

# Display the available development entry points.
help:
	@echo "Available targets:"
	@echo "  make run:dev                      Start all development services."
	@echo "  make run:dev dashboard           Start only the dashboard service."
	@echo "  make run:dev '#gateway'          Start all services except gateway."
	@echo "  make run:dev APP=dashboard       Start a specific service through a variable."
	@echo "  make run:dev EXCLUDE=gateway     Start all services except gateway through a variable."
	@echo ""
	@echo "  make infra:up                    Start all Docker infrastructure services (Kratos, Keto, ...)."
	@echo "  make infra:down                  Stop all Docker infrastructure services."
	@echo "  make infra:logs                  Tail logs from all infrastructure services."
	@echo ""
	@echo "  make admin EMAIL=a@b.com         Grant the admin role to a registered user."

# İlk admini yetkilendirir.
#
# /roles sayfasından yetki verilebiliyor, ama o sayfa da admin kapısının
# arkasında. Keto boşken hiç kimse giremediği için ilk üyelik dışarıdan
# yazılmak zorunda; sonrakiler arayüzden yapılabilir.
admin:
	@test -n "$(EMAIL)" || { echo "kullanım: make admin EMAIL=eposta@ornek.com" >&2; exit 1; }
	@./scripts/grant-admin.sh "$(EMAIL)"

# Docker altyapısı — platform/docker/compose.yaml üzerinden yönetilir.
COMPOSE_FILE := platform/docker/compose.yaml

infra\:up:
	@echo "Starting infrastructure services..."
	docker compose -f $(COMPOSE_FILE) up -d
	@echo "Infrastructure is up. Services:"
	@echo "  Kratos Public  → http://127.0.0.1:4433"
	@echo "  Kratos Admin   → http://127.0.0.1:4434"
	@echo "  Keto Read      → http://127.0.0.1:4466"
	@echo "  Keto Write     → http://127.0.0.1:4467"
	@echo "  MailSlurper    → http://127.0.0.1:4436"
	@echo "  And More..."

infra\:down:
	@echo "Stopping infrastructure services..."
	docker compose -f $(COMPOSE_FILE) down

infra\:logs:
	docker compose -f $(COMPOSE_FILE) logs -f

# Start the development environment using Docker.
# Usage examples:
#   make run:dev
#   make run:dev dashboard
#   make run:dev '#gateway'
#   make run:dev APP=dashboard
#   make run:dev EXCLUDE=gateway
run\:dev:
	@:
	@set -e; \
	selected="$(APP)"; \
	excluded="$(EXCLUDE)"; \
	goal_list="$(filter-out run:dev,$(MAKECMDGOALS))"; \
	for goal in $$goal_list; do \
		case "$$goal" in \
			\#*) excluded="$${goal#\#}" ;; \
			*) if [ -z "$$selected" ]; then selected="$$goal"; else echo "Ignoring extra selector: $$goal" >&2; fi ;; \
		esac; \
	done; \
	if [ -z "$$selected" ]; then \
		selected_apps="$(DEV_APPS)"; \
	else \
		selected_apps="$$selected"; \
	fi; \
	if [ -n "$$excluded" ]; then \
		selected_apps="$$(printf '%s\n' "$$selected_apps" | tr ' ' '\n' | grep -vx "$$excluded" | tr '\n' ' ' | sed 's/ *$$//')"; \
	fi; \
	echo "Starting Docker services: $$selected_apps"; \
	docker compose -f $(COMPOSE_FILE) up -d $$selected_apps

# Shortcut for running cargo watch natively with .cargo/config.toml variables
# Usage: make watch APP=humanizer
watch:
ifndef APP
	$(error Lütfen bir servis adı verin: make watch APP=humanizer)
endif
	@echo "Lokal geliştirme modu (cargo watch) başlatılıyor: $(APP)"
	@docker compose -f $(COMPOSE_FILE) stop $(APP)
	cargo watch -w apps/$(if $(filter vision humanizer orchestrator sonic,$(APP)),ai/$(APP),$(if $(filter dashboard,$(APP)),dashboard,$(APP))) -w packages -x "run -p $(APP)"