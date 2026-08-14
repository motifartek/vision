SHELL := /bin/sh

.PHONY: help run\:dev dashboard gateway identity stream

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

# Start the development environment.
#
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
	mkdir -p .make/logs; \
	log_dir="$(CURDIR)/.make/logs"; \
	for app in $$selected_apps; do \
		case "$$app" in \
			dashboard) \
				echo "Starting dashboard in development mode..."; \
				(cd "$(CURDIR)/apps/dashboard" && nohup pnpm dev > "$$log_dir/dashboard.log" 2>&1 &) ;; \
			gateway) \
				echo "Starting gateway in development mode..."; \
				(cd "$(CURDIR)/apps/gateway" && nohup cargo run > "$$log_dir/gateway.log" 2>&1 &) ;; \
			identity) \
				echo "Starting identity in development mode..."; \
				(cd "$(CURDIR)/apps/identity" && nohup cargo run > "$$log_dir/identity.log" 2>&1 &) ;; \
			stream) \
				echo "Starting stream in development mode..."; \
				(cd "$(CURDIR)/apps/stream" && nohup cargo run > "$$log_dir/stream.log" 2>&1 &) ;; \
			*) \
				echo "Unknown development application: $$app" >&2; \
				exit 1 ;; \
		esac; \
	done; \
	echo "Development services launched. Logs are available under $$log_dir."
