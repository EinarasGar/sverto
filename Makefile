# Myra Personal Finance
# ================================================

-include .env

# Colors for output
GREEN = \033[0;32m
YELLOW = \033[1;33m
RED = \033[0;31m
NC = \033[0m # No Color

# PIDs of THIS worktree's worker processes (the worker has no port). The worker binary's
# argv is the relative path "target/debug/worker", identical across worktrees, so we match
# the command and then confirm each process's cwd is inside this worktree before acting on
# it — otherwise we'd touch workers belonging to other worktrees.
# The patterns bracket their first char ([t]arget, [c]argo) so this very command line — which
# contains the patterns — does not self-match pgrep (the classic `ps | grep [p]attern` trick).
WORKER_PIDS = $$({ pgrep -f "[t]arget/debug/worker"; pgrep -f "[c]argo run -p worker"; } 2>/dev/null | sort -u | while read -r pid; do cwd=$$(lsof -a -d cwd -p $$pid -Fn 2>/dev/null | sed -n 's/^n//p'); case "$$cwd" in ("$(CURDIR)"|"$(CURDIR)"/*) echo $$pid ;; esac; done)

# Device the emulator/simulator targets use: phone, fold or tablet, named after the .env name plus -fold / -tablet.
# Plain `=` so only the command line can change it, not a DEVICE variable left in the environment.
DEVICE = phone
DEVICE_SUFFIX = $(if $(filter phone,$(DEVICE)),,-$(DEVICE))
CHECK_DEVICE = case "$(DEVICE)" in phone|fold|tablet) ;; *) echo "$(RED)Error: DEVICE must be phone, fold or tablet (got '$(DEVICE)')$(NC)"; exit 1 ;; esac

ANDROID_SDK = $(HOME)/Library/Android/sdk
ADB = $(ANDROID_SDK)/platform-tools/adb
EMULATOR = $(ANDROID_SDK)/emulator/emulator
AVDMANAGER = $(ANDROID_SDK)/cmdline-tools/latest/bin/avdmanager
SDKMANAGER = $(ANDROID_SDK)/cmdline-tools/latest/bin/sdkmanager
ANDROID_SYSTEM_IMAGE ?= system-images;android-37.0;google_apis_playstore_ps16k;arm64-v8a
AVD_NAME = $(ANDROID_AVD_NAME)$(DEVICE_SUFFIX)
AVD_PROFILE_phone = pixel_9
AVD_PROFILE_fold = pixel_9_pro_fold
AVD_PROFILE_tablet = pixel_tablet
AVD_PROFILE = $(AVD_PROFILE_$(DEVICE))
avd_dir = $(HOME)/.android/avd/$(1).avd
AVD_DIR = $(call avd_dir,$(AVD_NAME))
# Serial of the running emulator that booted the AVD named $(1) (empty if none). Resolved by AVD
# name through the emulator console, not by port, so it also finds the AVD when Android Studio
# launched it on whatever port was free.
avd_serial = $$(for s in $$($(ADB) devices 2>/dev/null | awk '/^emulator-/ {print $$1}'); do if [ "$$($(ADB) -s $$s emu avd name 2>/dev/null | head -n1 | tr -d '\r')" = "$(1)" ]; then echo $$s; break; fi; done)
AVD_SERIAL = $(call avd_serial,$(AVD_NAME))
PHYSICAL_SERIALS = $$($(ADB) devices 2>/dev/null | awk '$$2 == "device" && $$1 !~ /^emulator-/ {print $$1}')

IOS_SIMULATOR_DEVICE_TYPE ?= iPhone 17 Pro
IOS_SIM_NAME = $(IOS_SIMULATOR_NAME)$(DEVICE_SUFFIX)
IOS_DEVICE_TYPE_phone = $(IOS_SIMULATOR_DEVICE_TYPE)
IOS_DEVICE_TYPE_fold = iPhone Duo
IOS_DEVICE_TYPE_tablet = iPad Pro 11-inch (M5)
IOS_DEVICE_TYPE = $(IOS_DEVICE_TYPE_$(DEVICE))
# Newest installed runtime that can run the device type: the iPhone Duo has a runtime of its own that runs nothing else.
IOS_SIMULATOR_RUNTIME ?= $$(xcrun simctl list runtimes -j | python3 -c 'import sys,json; rs=[r for r in json.load(sys.stdin)["runtimes"] if r["platform"]=="iOS" and r["isAvailable"] and any(t["name"]==sys.argv[1] for t in r["supportedDeviceTypes"])]; rs.sort(key=lambda r: [int(x) for x in r["version"].split(".")]); print(rs[-1]["identifier"] if rs else "")' "$(IOS_DEVICE_TYPE)")
# Xcode for DEVICE=fold: an app adapts to the iPhone Duo only when built with the iOS 27.1 SDK or later. The selected
# Xcode if it has one, else the first /Applications/Xcode*.app that does; phone and tablet always use the selected Xcode.
# Override with IOS_FOLD_XCODE=/Applications/Xcode-27.2.0.app (an Xcode.app or its Contents/Developer).
IOS_FOLD_XCODE ?= $$(for d in "$$(xcode-select -p)" /Applications/Xcode*.app; do v=$$(DEVELOPER_DIR="$$d" xcrun --sdk iphonesimulator --show-sdk-version 2>/dev/null) && [ "$$(printf '%s\n' 27.1 "$$v" | sort -V | head -n1)" = 27.1 ] && { echo "$$d"; break; }; done)
IOS_XCODE_fold = XC="$(IOS_FOLD_XCODE)"; test -n "$$XC" || { echo "$(RED)Error: DEVICE=fold needs Xcode 27.1 or later for the iPhone Duo simulator. Install it beside the selected Xcode (no need to select it) with the iPhone Duo runtime, or set IOS_FOLD_XCODE=/path/to/Xcode.app$(NC)"; exit 1; }; export DEVELOPER_DIR="$$(DEVELOPER_DIR="$$XC" xcode-select -p)";
IOS_XCODE = $(IOS_XCODE_$(DEVICE))
# UDID / state of the simulator named $(1) (empty if it does not exist yet); resolved by name like avd_serial.
sim_udid = $$(xcrun simctl list devices -j | python3 -c 'import sys,json; ds=[d for v in json.load(sys.stdin)["devices"].values() for d in v if d["name"]==sys.argv[1] and d.get("isAvailable",True)]; print(ds[0]["udid"] if ds else "")' "$(1)")
sim_state = $$(xcrun simctl list devices -j | python3 -c 'import sys,json; ds=[d for v in json.load(sys.stdin)["devices"].values() for d in v if d["name"]==sys.argv[1]]; print(ds[0]["state"] if ds else "")' "$(1)")
SIM_UDID = $(call sim_udid,$(IOS_SIM_NAME))
SIM_STATE = $(call sim_state,$(IOS_SIM_NAME))
IOS_DIR = ios
IOS_PROJECT = $(IOS_DIR)/Sverto.xcodeproj
IOS_DERIVED = $(IOS_DIR)/build
# The fold Xcode builds into its own DerivedData, so switching between DEVICE=fold and the others does not make
# either Xcode rebuild everything (or trip over modules the other compiler wrote).
IOS_DEVICE_DERIVED = $(if $(filter fold,$(DEVICE)),$(IOS_DERIVED)/fold,$(IOS_DERIVED))
IOS_DEV_BUNDLE_ID = com.sverto.app.dev
IOS_PROD_BUNDLE_ID = com.sverto.app

# Default target
.PHONY: help
help: ## Show this help message
	@echo "$(GREEN)Myra - Available Commands$(NC)"
	@echo "=================================="
	@grep -hE '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "$(YELLOW)%-20s$(NC) %s\n", $$1, $$2}'

# Setup
auth ?= noauth
telemetry ?= local
secrets ?= db
LANDING_SITE_URL ?= https://sverto.com
LANDING_APP_URL ?= https://app.sverto.com
# Port scheme: 2<PREFIX><SS> -> 20000-29999. PREFIX = 2-digit worktree id (00 = main,
# 01-99 = worktree hash); SS = 2-digit service slot (00-99, 100 slots). Add a service
# by giving it the next free SS. Stays clear of macOS-reserved 5000/7000 and the
# ephemeral range (49152+).
.PHONY: setup-env
setup-env: ## Create .env file (worktree-aware). Use auth=noauth|database|clerk telemetry=local|axiom secrets=db|vault
	@if [ "$$(git rev-parse --git-common-dir 2>/dev/null)" != "$$(git rev-parse --git-dir 2>/dev/null)" ]; then \
		WORKTREE_NAME=$$(basename $$(pwd)); \
		HASH=$$(printf '%s' "$$WORKTREE_NAME" | cksum | awk '{print $$1}'); \
		PREFIX=$$(printf '%02d' $$((HASH % 99 + 1))); \
		AVD_NAME="myra-$$(printf '%s' "$$WORKTREE_NAME" | tr -c 'A-Za-z0-9._-' '_')"; \
		echo "$(GREEN)Worktree detected: $$WORKTREE_NAME (port prefix: $$PREFIX, AVD: $$AVD_NAME)$(NC)"; \
	else \
		PREFIX="00"; \
		AVD_NAME="myra-main"; \
	fi; \
	case "$(telemetry)" in \
		local) \
			OTLP_TRACES_ENDPOINT_VALUE="http://localhost:2$${PREFIX}04/v1/traces"; \
			OTLP_LOGS_ENDPOINT_VALUE="http://localhost:2$${PREFIX}00/ingest/otlp/v1/logs"; \
			;; \
		axiom) \
			OTLP_TRACES_ENDPOINT_VALUE="https://us-east-1.aws.edge.axiom.co/v1/traces"; \
			OTLP_LOGS_ENDPOINT_VALUE="https://us-east-1.aws.edge.axiom.co/v1/logs"; \
			;; \
		*) \
			echo "$(RED)Error: Unknown telemetry provider '$(telemetry)'. Use local or axiom.$(NC)"; \
			exit 1; \
			;; \
	esac; \
	printf '%s\n' \
		"POSTGRES_USER=myradev" \
		"POSTGRES_PASSWORD=devpassword" \
		"POSTGRES_DB=myra" \
		"RUST_LOG=info,api=debug,business=debug,dal=debug,worker=debug,ai=debug,tower_http=info,rig=info,hyper=warn,h2=warn,reqwest=warn,sqlx=warn" \
		"JWT_SECRET=devjwtsecret" \
		"" \
		"POSTGRES_PORT=2$${PREFIX}01" \
		"SERVER_PORT=2$${PREFIX}02" \
		"VITE_PORT=2$${PREFIX}03" \
		"LANDING_PORT=2$${PREFIX}11" \
		"OTLP_PORT=2$${PREFIX}04" \
		"OTLP_TRACES_ENDPOINT=$${OTLP_TRACES_ENDPOINT_VALUE}" \
		"OTLP_LOGS_ENDPOINT=$${OTLP_LOGS_ENDPOINT_VALUE}" \
		"JAEGER_UI_PORT=2$${PREFIX}05" \
		"SEQ_PORT=2$${PREFIX}00" \
		"COOKIE_SECURE=false" \
		"S3_PORT=2$${PREFIX}06" \
		"S3_CONSOLE_PORT=2$${PREFIX}07" \
		"REDIS_PORT=2$${PREFIX}08" \
		"REDIS_URL=redis://localhost:2$${PREFIX}08" \
		"VAULT_PORT=2$${PREFIX}10" \
		"VAULT_TOKEN=dev-token" \
		"MARKET_DATA_PORT=2$${PREFIX}09" \
		"MARKET_DATA_URL=http://localhost:2$${PREFIX}09" \
		"MARKET_DATA_API_KEY=dev-market-data-key" \
		"S3_ENDPOINT=http://localhost:2$${PREFIX}06" \
		"S3_BUCKET_NAME=myra-files" \
		"S3_ACCESS_KEY=rustfsadmin" \
		"S3_SECRET_KEY=rustfsadmin123" \
		"S3_REGION=us-east-1" \
		> .env; \
	printf '\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n' \
		"AI_PROVIDER=gemini" \
		"AI_MODEL=gemini-3.8-flash" \
		"AI_EMBEDDING_PROVIDER=gemini" \
		"AI_EMBEDDING_MODEL=gemini-embedding-2-preview" \
		"IMPORT_CLASSIFIER=llm" \
		"SYSTEM_ONE_PROVIDER=typesafe" \
		"SYSTEM_ONE_MODEL=jev-1.14.0" \
		>> .env; \
	SECRETS_FILE=".secrets.dev"; \
	if [ ! -f "$$SECRETS_FILE" ]; then \
		echo "$(RED)Error: $$SECRETS_FILE not found. Copy .secrets.example to $$SECRETS_FILE and fill in values.$(NC)"; \
		exit 1; \
	fi; \
	. ./$$SECRETS_FILE; \
	printf '%s\n%s\n%s\n' \
		"AI_API_KEY=$${AI_API_KEY}" \
		"AI_EMBEDDING_API_KEY=$${AI_EMBEDDING_API_KEY:-$${AI_API_KEY}}" \
		"SYSTEM_ONE_KEY=$${SYSTEM_ONE_KEY}" \
		>> .env; \
	if [ -n "$${TRUELAYER_CLIENT_ID}" ]; then \
		printf '\n%s\n%s\n%s\n%s\n%s\n' \
			"# TrueLayer" \
			"TRUELAYER_CLIENT_ID=$${TRUELAYER_CLIENT_ID}" \
			"TRUELAYER_CLIENT_SECRET=$${TRUELAYER_CLIENT_SECRET}" \
			"TRUELAYER_ENV=$${TRUELAYER_ENV}" \
			"TRUELAYER_REDIRECT_URI_ALLOWLIST=http://localhost:2$${PREFIX}03/settings/connectors/truelayer/callback,$${TRUELAYER_REDIRECT_URI_ALLOWLIST}" \
			>> .env; \
	fi; \
	if [ -n "$${ENABLEBANKING_APPLICATION_ID}" ]; then \
		printf '\n%s\n%s\n%s\n%s\n' \
			"# Enable Banking" \
			"ENABLEBANKING_APPLICATION_ID=$${ENABLEBANKING_APPLICATION_ID}" \
			"ENABLEBANKING_PRIVATE_KEY=\"$${ENABLEBANKING_PRIVATE_KEY}\"" \
			"ENABLEBANKING_REDIRECT_URI_ALLOWLIST=http://localhost:2$${PREFIX}02/connectors/enablebanking/callback,http://localhost:2$${PREFIX}03/settings/connectors/enablebanking/callback,$${ENABLEBANKING_REDIRECT_URI_ALLOWLIST}" \
			>> .env; \
	fi; \
	if [ "$(telemetry)" = "axiom" ]; then \
		if [ -z "$${AXIOM_API_TOKEN}" ]; then \
			echo "$(RED)Error: AXIOM_API_TOKEN not found in $$SECRETS_FILE$(NC)"; \
			exit 1; \
		fi; \
		printf '\n%s\n%s\n%s\n' \
			"# Axiom telemetry" \
			"OTLP_AUTH_TOKEN=$${AXIOM_API_TOKEN}" \
			"AXIOM_DATASET=sverto-dev" \
			>> .env; \
	fi; \
	case "$(auth)" in \
		noauth|database) \
			printf '\n%s\n' "AUTH_PROVIDER=$(auth)" >> .env; \
			;; \
		clerk) \
			if [ -z "$$CLERK_PUBLISHABLE_KEY" ]; then \
				echo "$(RED)Error: CLERK_PUBLISHABLE_KEY not found in $$SECRETS_FILE$(NC)"; \
				exit 1; \
			fi; \
			printf '\n%s\n\n%s\n%s\n%s\n' \
				"AUTH_PROVIDER=clerk" \
				"# Required when AUTH_PROVIDER=clerk" \
				"CLERK_PUBLISHABLE_KEY=$$CLERK_PUBLISHABLE_KEY" \
				"CLERK_SECRET_KEY=$$CLERK_SECRET_KEY" \
				>> .env; \
			;; \
		*) \
			echo "$(RED)Error: Unknown auth provider '$(auth)'. Use noauth, database, or clerk.$(NC)"; \
			exit 1; \
			;; \
	esac; \
	case "$(secrets)" in \
		db) \
			printf '\n%s\n%s\n' \
				"SECRET_PROVIDER=local_encrypted" \
				"CONNECTOR_ENC_KEY=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" \
				>> .env; \
			;; \
		vault) \
			printf '\n%s\n%s\n' \
				"SECRET_PROVIDER=vault" \
				"VAULT_ADDR=http://localhost:2$${PREFIX}10" \
				>> .env; \
			;; \
		*) \
			echo "$(RED)Error: Unknown secrets provider '$(secrets)'. Use db or vault.$(NC)"; \
			exit 1; \
			;; \
	esac; \
	printf '\n%s\n%s\n%s\n%s\n%s\n' \
		"# Android signing" \
		"SVERTO_STORE_FILE=$${SVERTO_STORE_FILE}" \
		"SVERTO_STORE_PASSWORD=$${SVERTO_STORE_PASSWORD}" \
		"SVERTO_KEY_ALIAS=$${SVERTO_KEY_ALIAS}" \
		"SVERTO_KEY_PASSWORD=$${SVERTO_KEY_PASSWORD}" \
		>> .env; \
	printf '\n%s\n%s\n' \
		"# Android emulator" \
		"ANDROID_AVD_NAME=$${AVD_NAME}" \
		>> .env; \
	printf '\n%s\n%s\n' \
		"# iOS simulator" \
		"IOS_SIMULATOR_NAME=$${AVD_NAME}" \
		>> .env; \
	printf '\n%s\n' "APP_ENV=dev" >> .env; \
	echo "$(GREEN).env created (auth=$(auth), telemetry=$(telemetry), secrets=$(secrets)):$(NC)"; \
	cat .env
	@echo ""
	@echo "$(GREEN)Installing UI dependencies...$(NC)"
	cd web && bun install
	@echo "$(GREEN)Installing landing dependencies...$(NC)"
	cd marketing/landing && bun install
	@echo "$(GREEN)Building Rust workspace...$(NC)"
	cd server && cargo build
	@echo "$(GREEN)Setup complete!$(NC)"

# Status
.PHONY: status
status: ## Show service ports, status, and useful links
	@echo "$(GREEN)Auth Provider:$(NC)    $(YELLOW)$(AUTH_PROVIDER)$(NC)"
	@echo "$(GREEN)Secret Provider:$(NC)  $(YELLOW)$(SECRET_PROVIDER)$(NC)"
	@PROJ=$(basename $(pwd)); \
	if docker volume inspect $${PROJ}_myra-postgres-data >/dev/null 2>&1; then \
		echo "$(GREEN)Database Volume:$(NC)  $(GREEN)Yes$(NC)"; \
	else \
		echo "$(GREEN)Database Volume:$(NC)  $(RED)No$(NC)"; \
	fi
	@echo ""
	@echo "$(GREEN)Links$(NC)"
	@echo "=================================="
	@echo "$(YELLOW)UI$(NC)              http://localhost:$(VITE_PORT)/"
	@echo "$(YELLOW)Landing$(NC)         http://localhost:$(LANDING_PORT)/"
	@echo "$(YELLOW)Redoc$(NC)           http://localhost:$(SERVER_PORT)/redoc"
	@echo "$(YELLOW)APIs$(NC)            http://localhost:$(SERVER_PORT)/api"
	@echo "$(YELLOW)Jaeger$(NC)          http://localhost:$(JAEGER_UI_PORT)/"
	@echo "$(YELLOW)Seq$(NC)             http://localhost:$(SEQ_PORT)/"
	@echo ""
	@echo "$(GREEN)Myra - Service Ports$(NC)"
	@echo "=================================="
	@DOCKER_PS=$$(timeout 3 docker-compose ps --format json 2>/dev/null || true); \
	check_docker() { \
		echo "$$DOCKER_PS" | grep -q "\"$$1\"" && echo "$$DOCKER_PS" | grep "\"$$1\"" | grep -qi '"running"'; \
	}; \
	check_infra() { \
		local name=$$1 port=$$2 service=$$3; \
		if check_docker $$service; then \
			echo "$(YELLOW)$$name$(NC)  http://localhost:$$port - $(GREEN)Running$(NC)"; \
		elif nc -z -G 1 localhost $$port 2>/dev/null; then \
			echo "$(YELLOW)$$name$(NC)  http://localhost:$$port - $(RED)Port in use (not docker!)$(NC)"; \
		else \
			echo "$(YELLOW)$$name$(NC)  http://localhost:$$port - $(RED)Not Running$(NC)"; \
		fi; \
	}; \
	check_local() { \
		local name=$$1 port=$$2; \
		if nc -z -G 1 localhost $$port 2>/dev/null; then \
			echo "$(YELLOW)$$name$(NC)  http://localhost:$$port - $(GREEN)Running$(NC)"; \
		else \
			echo "$(YELLOW)$$name$(NC)  http://localhost:$$port - $(RED)Not Running$(NC)"; \
		fi; \
	}; \
	check_worker() { \
		if [ -n "$(WORKER_PIDS)" ]; then \
			echo "$(YELLOW)Worker        $(NC)  (no port) - $(GREEN)Running$(NC)"; \
		else \
			echo "$(YELLOW)Worker        $(NC)  (no port) - $(RED)Not Running$(NC)"; \
		fi; \
	}; \
	check_emulator() { \
		SERIAL="$(call avd_serial,$$1)"; \
		if [ -n "$$SERIAL" ]; then \
			echo "$(YELLOW)Emulator      $(NC)  $$1 ($$SERIAL) - $(GREEN)Running$(NC)"; \
		else \
			echo "$(YELLOW)Emulator      $(NC)  $$1 - $(RED)Not Running$(NC)"; \
		fi; \
	}; \
	check_simulator() { \
		UDID="$(call sim_udid,$$1)"; STATE="$(call sim_state,$$1)"; \
		if [ "$$STATE" = "Booted" ]; then \
			echo "$(YELLOW)Simulator     $(NC)  $$1 ($$UDID) - $(GREEN)Running$(NC)"; \
		elif [ -n "$$UDID" ]; then \
			echo "$(YELLOW)Simulator     $(NC)  $$1 ($$UDID) - $(RED)Not Running$(NC)"; \
		else \
			echo "$(YELLOW)Simulator     $(NC)  $$1 - $(RED)Not Created$(NC)"; \
		fi; \
	}; \
	check_infra "Postgres      " $(POSTGRES_PORT) database; \
	check_local "API Server    " $(SERVER_PORT); \
	check_worker; \
	check_local "Market Data   " $(MARKET_DATA_PORT); \
	check_local "Vite Dev      " $(VITE_PORT); \
	check_local "Landing Dev   " $(LANDING_PORT); \
	check_infra "OTLP Collector" $(OTLP_PORT) jaeger; \
	check_infra "Jaeger UI     " $(JAEGER_UI_PORT) jaeger; \
	check_infra "Seq Logs      " $(SEQ_PORT) seq; \
	check_infra "RustFS        " $(S3_PORT) rustfs; \
	check_infra "RustFS Console" $(S3_CONSOLE_PORT) rustfs; \
	check_infra "Redis         " $(REDIS_PORT) redis; \
	check_infra "Vault         " $(VAULT_PORT) vault; \
	check_emulator "$(ANDROID_AVD_NAME)"; \
	for d in fold tablet; do if [ -d "$(call avd_dir,$(ANDROID_AVD_NAME)-$$d)" ]; then check_emulator "$(ANDROID_AVD_NAME)-$$d"; fi; done; \
	check_simulator "$(IOS_SIMULATOR_NAME)"; \
	for d in fold tablet; do if [ -n "$(call sim_udid,$(IOS_SIMULATOR_NAME)-$$d)" ]; then check_simulator "$(IOS_SIMULATOR_NAME)-$$d"; fi; done

# Run
.PHONY: backend-run
backend-run: ## Start API server (kills existing process on SERVER_PORT first)
	-@lsof -ti :$(SERVER_PORT) | xargs kill -9 2>/dev/null || true
	cd server && cargo run -p api --no-default-features --features $(AUTH_PROVIDER),seed

.PHONY: worker-run
worker-run: ## Start background worker (kills existing worker first). Shares this worktree's .env — no port needed.
	-@PIDS="$(WORKER_PIDS)"; [ -n "$$PIDS" ] && kill $$PIDS 2>/dev/null || true
	cd server && cargo run -p worker

.PHONY: market-data-run
market-data-run: ## Start market data API (kills existing process on MARKET_DATA_PORT first)
	-@lsof -ti :$(MARKET_DATA_PORT) | xargs kill -9 2>/dev/null || true
	cd market-data-api && cargo run

.PHONY: web-run
web-run: ## Start Vite dev server (kills existing process on VITE_PORT first)
	-@lsof -ti :$(VITE_PORT) | xargs kill -9 2>/dev/null || true
	cd web && bun run dev

.PHONY: landing-run
landing-run: ## Start Astro dev server for the marketing site (kills LANDING_PORT first; astro dev daemonises, stop it with "cd marketing/landing && bunx astro dev stop")
	-@lsof -ti :$(LANDING_PORT) | xargs kill -9 2>/dev/null || true
	cd marketing/landing && \
		PUBLIC_SITE_URL=http://localhost:$(LANDING_PORT) \
		PUBLIC_APP_URL=http://localhost:$(VITE_PORT) \
		bun run dev --port $(LANDING_PORT)

.PHONY: landing-build
landing-build: ## Build the static marketing site into marketing/landing/dist
	cd marketing/landing && \
		PUBLIC_SITE_URL=$(LANDING_SITE_URL) \
		PUBLIC_APP_URL=$(LANDING_APP_URL) \
		bun run build

.PHONY: ide
ide: ## Open VS Code and auto-start infra, backend, worker, market-data, web in split terminals
	@code .vscode/myra.code-workspace

.PHONY: android-emulator
android-emulator: ## Boot this worktree's AVD, creating it on first use. DEVICE=phone (default, Pixel 9), fold (Pixel 9 Pro Fold) or tablet (Pixel Tablet); reset=true recreates it from scratch (wipes its data)
	@test -n "$(ANDROID_AVD_NAME)" || { echo "$(RED)Error: ANDROID_AVD_NAME is not set. Run make setup-env first.$(NC)"; exit 1; }
	@$(CHECK_DEVICE)
	@SERIAL="$(AVD_SERIAL)"; \
	if [ "$(reset)" = "true" ] && [ -n "$$SERIAL" ]; then \
		echo "$(YELLOW)Stopping $(AVD_NAME) ($$SERIAL) for reset...$(NC)"; \
		$(ADB) -s "$$SERIAL" emu kill >/dev/null; \
		while $(ADB) devices | grep -qw "$$SERIAL"; do sleep 1; done; \
		SERIAL=""; \
	fi; \
	if [ -z "$$SERIAL" ]; then \
		if [ "$(reset)" = "true" ] || [ ! -d "$(AVD_DIR)" ]; then \
			if [ ! -d "$(ANDROID_SDK)/$(subst ;,/,$(ANDROID_SYSTEM_IMAGE))" ]; then \
				echo "$(GREEN)Installing $(ANDROID_SYSTEM_IMAGE)...$(NC)"; \
				yes | $(SDKMANAGER) --install "$(ANDROID_SYSTEM_IMAGE)"; \
			fi; \
			{ echo no | $(AVDMANAGER) create avd --force -n "$(AVD_NAME)" -k "$(ANDROID_SYSTEM_IMAGE)" -d $(AVD_PROFILE) --skin $(AVD_PROFILE) && \
			sed -i '' \
				-e 's/^PlayStore.enabled=.*/PlayStore.enabled=true/' \
				-e 's/^hw.keyboard=.*/hw.keyboard=yes/' \
				-e 's/^hw.gpu.enabled=.*/hw.gpu.enabled=yes/' \
				-e 's/^hw.camera.back=.*/hw.camera.back=virtualscene/' \
				-e 's/^hw.camera.front=.*/hw.camera.front=emulated/' \
				-e 's/^disk.dataPartition.size=.*/disk.dataPartition.size=20G/' \
				-e 's/^sdcard.size=.*/sdcard.size=6G/' \
				"$(AVD_DIR)/config.ini" && \
			printf '%s\n%s\n' "avd.ini.displayname=$(AVD_NAME)" "AvdId=$(AVD_NAME)" >> "$(AVD_DIR)/config.ini"; } || exit 1; \
			echo "$(GREEN)AVD $(AVD_NAME) created$(NC)"; \
		fi; \
		echo "$(GREEN)Booting $(AVD_NAME)...$(NC)"; \
		nohup $(EMULATOR) -avd "$(AVD_NAME)" </dev/null >"$(AVD_DIR)/emulator.log" 2>&1 & \
		i=0; \
		until SERIAL="$(AVD_SERIAL)"; [ -n "$$SERIAL" ]; do \
			i=$$((i + 1)); \
			if [ $$i -gt 60 ]; then echo "$(RED)Error: $(AVD_NAME) did not show up in adb within 60s, see $(AVD_DIR)/emulator.log$(NC)"; exit 1; fi; \
			sleep 1; \
		done; \
	fi; \
	i=0; \
	until [ "$$($(ADB) -s "$$SERIAL" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = "1" ]; do \
		i=$$((i + 1)); \
		if [ $$i -gt 150 ]; then echo "$(RED)Error: $$SERIAL did not finish booting within 300s$(NC)"; exit 1; fi; \
		sleep 2; \
	done; \
	echo "$(GREEN)$(AVD_NAME) ready on $$SERIAL$(NC)"

.PHONY: android-run
android-run: android-emulator ## Build, install, and launch the dev Android app on this worktree's emulator (boots it if needed; DEVICE=phone|fold|tablet)
	@SERIAL="$(AVD_SERIAL)"; \
	cd android && ANDROID_SERIAL="$$SERIAL" ./gradlew installDevDebug && \
	$(ADB) -s "$$SERIAL" reverse tcp:$(SERVER_PORT) tcp:$(SERVER_PORT) >/dev/null && \
	$(ADB) -s "$$SERIAL" reverse tcp:$(S3_PORT) tcp:$(S3_PORT) >/dev/null && \
	echo "$(GREEN)Launching on $$SERIAL$(NC)" && \
	$(ADB) -s "$$SERIAL" shell am start -n com.sverto.app.dev/com.sverto.app.MainActivity

.PHONY: android-install-prod
android-install-prod: android-emulator ## Build and install the prod Android app on this worktree's emulator (DEVICE=phone|fold|tablet) and any connected physical devices (points at api.sverto.com)
	@if [ ! -f ".secrets.prod" ]; then \
		echo "$(RED)Error: .secrets.prod not found.$(NC)"; \
		exit 1; \
	fi; \
	. ./.secrets.prod; \
	SERIALS="$(AVD_SERIAL)"; \
	for p in $(PHYSICAL_SERIALS); do SERIALS="$$SERIALS,$$p"; done; \
	echo "$(GREEN)Installing on: $$SERIALS$(NC)"; \
	cd android && ANDROID_SERIAL="$$SERIALS" ./gradlew installProdRelease \
		-PAPP_ENV=prod \
		-PAPP_API_BASE_URL=https://api.sverto.com \
		-PCLERK_PUBLISHABLE_KEY="$$CLERK_PUBLISHABLE_KEY" \
		-PSVERTO_STORE_FILE="$$SVERTO_STORE_FILE" \
		-PSVERTO_STORE_PASSWORD="$$SVERTO_STORE_PASSWORD" \
		-PSVERTO_KEY_ALIAS="$$SVERTO_KEY_ALIAS" \
		-PSVERTO_KEY_PASSWORD="$$SVERTO_KEY_PASSWORD"

.PHONY: android-bundle-prod
android-bundle-prod: ## Build the prod release App Bundle (AAB) for Google Play upload
	@if [ ! -f ".secrets.prod" ]; then \
		echo "$(RED)Error: .secrets.prod not found.$(NC)"; \
		exit 1; \
	fi; \
	. ./.secrets.prod; \
	cd android && ./gradlew bundleProdRelease \
		-PAPP_ENV=prod \
		-PAPP_API_BASE_URL=https://api.sverto.com \
		-PCLERK_PUBLISHABLE_KEY="$$CLERK_PUBLISHABLE_KEY" \
		-PSVERTO_STORE_FILE="$$SVERTO_STORE_FILE" \
		-PSVERTO_STORE_PASSWORD="$$SVERTO_STORE_PASSWORD" \
		-PSVERTO_KEY_ALIAS="$$SVERTO_KEY_ALIAS" \
		-PSVERTO_KEY_PASSWORD="$$SVERTO_KEY_PASSWORD"
	@echo "$(GREEN)AAB: android/app/build/outputs/bundle/prodRelease/app-prod-release.aab$(NC)"

# iOS
.PHONY: ios-core
ios-core: ## Build the Rust core for the iOS simulator and package SvertoCoreFFI.xcframework + Swift bindings (variant=all adds the device slice; FORCE=1 rebuilds)
	@$(IOS_DIR)/scripts/build-core.sh $(or $(variant),sim)

.PHONY: ios-lucide
ios-lucide: ## Generate the Lucide icon asset catalog used for server-provided category icons
	@$(IOS_DIR)/scripts/make-lucide-assets.sh

.PHONY: ios-project
ios-project: ## Regenerate ios/Sverto.xcodeproj from ios/project.yml with xcodegen
	@command -v xcodegen >/dev/null || { echo "$(RED)Error: xcodegen not installed (brew install xcodegen)$(NC)"; exit 1; }
	@cd $(IOS_DIR) && xcodegen generate --quiet

.PHONY: ios-simulator
ios-simulator: ## Boot this worktree's simulator, creating it on first use. DEVICE=phone (default, iPhone 17 Pro), fold (iPhone Duo, with Xcode 27.1+, see IOS_FOLD_XCODE) or tablet (iPad Pro 11-inch); reset=true recreates it from scratch (wipes its data)
	@test -n "$(IOS_SIMULATOR_NAME)" || { echo "$(RED)Error: IOS_SIMULATOR_NAME is not set. Run make setup-env first.$(NC)"; exit 1; }
	@$(CHECK_DEVICE)
	@$(IOS_XCODE) UDID="$(SIM_UDID)"; \
	if [ "$(reset)" = "true" ] && [ -n "$$UDID" ]; then \
		echo "$(YELLOW)Deleting $(IOS_SIM_NAME) ($$UDID) for reset...$(NC)"; \
		xcrun simctl shutdown "$$UDID" >/dev/null 2>&1 || true; \
		xcrun simctl delete "$$UDID"; \
		UDID=""; \
	fi; \
	if [ -z "$$UDID" ]; then \
		RUNTIME="$(IOS_SIMULATOR_RUNTIME)"; \
		test -n "$$RUNTIME" || { echo "$(RED)Error: no iOS simulator runtime for $(IOS_DEVICE_TYPE) installed (Xcode > Settings > Components)$(NC)"; exit 1; }; \
		echo "$(GREEN)Creating $(IOS_SIM_NAME) ($(IOS_DEVICE_TYPE), $$RUNTIME)...$(NC)"; \
		UDID=$$(xcrun simctl create "$(IOS_SIM_NAME)" "$(IOS_DEVICE_TYPE)" "$$RUNTIME"); \
	fi; \
	if [ "$(SIM_STATE)" != "Booted" ]; then \
		echo "$(GREEN)Booting $(IOS_SIM_NAME)...$(NC)"; \
		xcrun simctl boot "$$UDID"; \
	fi; \
	xcrun simctl bootstatus "$$UDID" -b >/dev/null; \
	DEVICE_HUB="$$(xcode-select -p)/../Applications/DeviceHub.app"; \
	if [ -d "$$DEVICE_HUB" ]; then open -g -a "$$DEVICE_HUB"; else open -g -a Simulator --args -CurrentDeviceUDID "$$UDID"; fi; \
	echo "$(GREEN)$(IOS_SIM_NAME) ready ($$UDID)$(NC)"

.PHONY: ios-run
ios-run: ios-simulator ios-core ios-lucide ## Build, install, and launch the dev iOS app on this worktree's simulator (boots it if needed; DEVICE=phone|fold|tablet)
	@$(IOS_DIR)/scripts/write-generated-xcconfig.sh
	@if [ ! -d "$(IOS_PROJECT)" ] || [ "$(IOS_DIR)/project.yml" -nt "$(IOS_PROJECT)/project.pbxproj" ]; then $(MAKE) ios-project; fi
	@$(IOS_XCODE) UDID="$(SIM_UDID)"; \
	set -o pipefail; \
	xcodebuild -project $(IOS_PROJECT) -scheme Sverto -configuration Debug \
		-destination "platform=iOS Simulator,id=$$UDID" -derivedDataPath $(IOS_DEVICE_DERIVED) \
		-quiet build && \
	APP=$$(find $(IOS_DEVICE_DERIVED)/Build/Products/Debug-iphonesimulator -maxdepth 1 -name 'Sverto.app' | head -n1) && \
	xcrun simctl install "$$UDID" "$$APP" && \
	echo "$(GREEN)Launching $(IOS_DEV_BUNDLE_ID) on $(IOS_SIM_NAME)$(NC)" && \
	xcrun simctl launch "$$UDID" $(IOS_DEV_BUNDLE_ID) >/dev/null

.PHONY: ios-install-prod
ios-install-prod: ios-simulator ios-lucide ## Build and install the prod iOS app (points at api.sverto.com) on this worktree's simulator (DEVICE=phone|fold|tablet) and any connected iPhone. Needs .secrets.prod with CLERK_PUBLISHABLE_KEY and IOS_TEAM_ID
	@if [ ! -f ".secrets.prod" ]; then echo "$(RED)Error: .secrets.prod not found.$(NC)"; exit 1; fi
	@. ./.secrets.prod; \
	test -n "$$IOS_TEAM_ID" || { echo "$(RED)Error: IOS_TEAM_ID missing from .secrets.prod$(NC)"; exit 1; }; \
	$(IOS_DIR)/scripts/build-core.sh all && \
	APP_API_BASE_URL=https://api.sverto.com CLERK_PUBLISHABLE_KEY="$$CLERK_PUBLISHABLE_KEY" IOS_TEAM_ID="$$IOS_TEAM_ID" $(IOS_DIR)/scripts/write-generated-xcconfig.sh && \
	$(MAKE) ios-project && \
	UDID="$(SIM_UDID)"; \
	set -o pipefail; \
	( $(IOS_XCODE) xcodebuild -project $(IOS_PROJECT) -scheme Sverto -configuration Release \
		-destination "platform=iOS Simulator,id=$$UDID" -derivedDataPath $(IOS_DEVICE_DERIVED) -quiet build && \
	APP=$$(find $(IOS_DEVICE_DERIVED)/Build/Products/Release-iphonesimulator -maxdepth 1 -name 'Sverto.app' | head -n1) && \
	xcrun simctl install "$$UDID" "$$APP" && xcrun simctl launch "$$UDID" $(IOS_PROD_BUNDLE_ID) >/dev/null ) && \
	DEVICE_JSON=$$(mktemp); \
	xcrun devicectl list devices -j "$$DEVICE_JSON" >/dev/null || { rm -f "$$DEVICE_JSON"; exit 1; }; \
	DEVICES=$$(python3 -c 'import sys,json; print(" ".join(d["hardwareProperties"]["udid"] for d in json.load(open(sys.argv[1]))["result"]["devices"] if d.get("connectionProperties",{}).get("tunnelState")=="connected" or (d.get("connectionProperties",{}).get("transportType")=="wired" and d.get("connectionProperties",{}).get("pairingState")=="paired")))' "$$DEVICE_JSON") || { rm -f "$$DEVICE_JSON"; exit 1; }; \
	rm -f "$$DEVICE_JSON"; \
	for d in $$DEVICES; do \
		xcodebuild -project $(IOS_PROJECT) -scheme Sverto -configuration Release -destination "platform=iOS,id=$$d" \
			-derivedDataPath $(IOS_DERIVED) -allowProvisioningUpdates -allowProvisioningDeviceRegistration -quiet build || exit $$?; \
		DEVAPP=$$(find $(IOS_DERIVED)/Build/Products/Release-iphoneos -maxdepth 1 -name 'Sverto.app' | head -n1) && \
		echo "$(GREEN)Installing on device $$d$(NC)"; \
		xcrun devicectl device install app --device "$$d" "$$DEVAPP" && xcrun devicectl device process launch --device "$$d" $(IOS_PROD_BUNDLE_ID) || exit $$?; \
	done

.PHONY: ios-archive-prod
ios-archive-prod: ios-lucide ## Archive the prod iOS app and export an App Store Connect IPA (ios/build/Sverto.xcarchive, ios/build/export/Sverto.ipa). Needs .secrets.prod with CLERK_PUBLISHABLE_KEY and IOS_TEAM_ID
	@if [ ! -f ".secrets.prod" ]; then echo "$(RED)Error: .secrets.prod not found.$(NC)"; exit 1; fi
	@. ./.secrets.prod; \
	test -n "$$IOS_TEAM_ID" || { echo "$(RED)Error: IOS_TEAM_ID missing from .secrets.prod$(NC)"; exit 1; }; \
	$(IOS_DIR)/scripts/build-core.sh all && \
	APP_API_BASE_URL=https://api.sverto.com CLERK_PUBLISHABLE_KEY="$$CLERK_PUBLISHABLE_KEY" IOS_TEAM_ID="$$IOS_TEAM_ID" $(IOS_DIR)/scripts/write-generated-xcconfig.sh && \
	$(MAKE) ios-project && \
	xcodebuild -project $(IOS_PROJECT) -scheme Sverto -configuration Release -destination 'generic/platform=iOS' \
		-derivedDataPath $(IOS_DERIVED) -archivePath $(IOS_DERIVED)/Sverto.xcarchive -allowProvisioningUpdates -quiet CODE_SIGNING_ALLOWED=NO archive && \
	python3 $(IOS_DIR)/scripts/prepare-archive-signing.py $(IOS_DERIVED)/Sverto.xcarchive && \
	sed "s/IOS_TEAM_ID/$$IOS_TEAM_ID/" $(IOS_DIR)/Config/ExportOptions.plist > $(IOS_DERIVED)/ExportOptions.plist && \
	xcodebuild -exportArchive -archivePath $(IOS_DERIVED)/Sverto.xcarchive -exportOptionsPlist $(IOS_DERIVED)/ExportOptions.plist \
		-exportPath $(IOS_DERIVED)/export -allowProvisioningUpdates -quiet
	@echo "$(GREEN)Archive: $(IOS_DERIVED)/Sverto.xcarchive$(NC)"
	@echo "$(GREEN)IPA: $(IOS_DERIVED)/export/Sverto.ipa$(NC)"

# Infrastructure
.PHONY: start-infra
start-infra: ## Start infrastructure services (Postgres, Jaeger, etc.)
	docker-compose up -d

.PHONY: refresh-infra
refresh-infra: ## Wipe volumes and restart infrastructure services
	docker-compose down -v
	docker-compose up -d

.PHONY: _stop-processes
_stop-processes:
	@echo "$(YELLOW)Stopping local processes...$(NC)"
	-@lsof -ti :$(SERVER_PORT) | xargs kill -9 2>/dev/null || true
	-@lsof -ti :$(MARKET_DATA_PORT) | xargs kill -9 2>/dev/null || true
	-@lsof -ti :$(VITE_PORT) | xargs kill -9 2>/dev/null || true
	-@lsof -ti :$(LANDING_PORT) | xargs kill -9 2>/dev/null || true
	-@PIDS="$(WORKER_PIDS)"; [ -n "$$PIDS" ] && kill -9 $$PIDS 2>/dev/null || true

.PHONY: stop
stop: _stop-processes ## Stop all dev processes and docker containers (keeps volumes)
	@echo "$(YELLOW)Stopping docker containers...$(NC)"
	docker-compose down

.PHONY: destroy
destroy: _stop-processes ## Stop all dev processes and docker containers, AND delete volumes
	@echo "$(YELLOW)Stopping docker containers and deleting volumes...$(NC)"
	docker-compose down -v

# Database
.PHONY: export-db
export-db: ## Export database data to db_dump.sql
	@echo "$(GREEN)Exporting database data...$(NC)"
	@PGPASSWORD=$(POSTGRES_PASSWORD) pg_dump -h localhost -p $(POSTGRES_PORT) -U $(POSTGRES_USER) -d $(POSTGRES_DB) --data-only --exclude-table='_sqlx_*' --disable-triggers > db_dump.sql
	@echo "$(GREEN)Database data exported to db_dump.sql$(NC)"

.PHONY: seed-demo
seed-demo: ## Load the demo/showcase dataset for the default user (idempotent; needs DB running)
	@echo "$(GREEN)Seeding demo showcase data...$(NC)"
	@PGPASSWORD=$(POSTGRES_PASSWORD) psql -h localhost -p $(POSTGRES_PORT) -U $(POSTGRES_USER) -d $(POSTGRES_DB) -v ON_ERROR_STOP=1 -q -f database/demo/showcase_data.sql
	@echo "$(GREEN)Demo data loaded. Run 'make worker-run' so investment prices populate from market data.$(NC)"

.PHONY: import-db
import-db: ## Import database data from db_dump.sql (truncates existing data first)
	@echo "$(YELLOW)Truncating existing data...$(NC)"
	@PGPASSWORD=$(POSTGRES_PASSWORD) psql -h localhost -p $(POSTGRES_PORT) -U $(POSTGRES_USER) -d $(POSTGRES_DB) -c \
		"DO \$$\$$ DECLARE r RECORD; BEGIN FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = 'public' AND tablename NOT LIKE '_sqlx_%') LOOP EXECUTE 'TRUNCATE TABLE ' || quote_ident(r.tablename) || ' CASCADE'; END LOOP; END \$$\$$;"
	@echo "$(GREEN)Importing database data...$(NC)"
	@PGPASSWORD=$(POSTGRES_PASSWORD) psql -h localhost -p $(POSTGRES_PORT) -U $(POSTGRES_USER) -d $(POSTGRES_DB) < db_dump.sql
	@echo "$(GREEN)Database data imported from db_dump.sql$(NC)"

# Store screenshots
.PHONY: store-shots
store-shots: ## Capture every device from a fresh demo database (myra_shots; the backend is pointed back at the dev database afterwards) and render the store screenshots into marketing/app-stores/. only="android-phone ios-duo-open ..." captures just those configurations
	@SERVER_PORT=$(SERVER_PORT) POSTGRES_PORT=$(POSTGRES_PORT) POSTGRES_USER=$(POSTGRES_USER) POSTGRES_PASSWORD=$(POSTGRES_PASSWORD) POSTGRES_DB=$(POSTGRES_DB) \
		marketing/app-stores/shots/run.sh $(only)

.PHONY: store-shots-compose
store-shots-compose: ## Re-render the store screenshots from the raw captures already on disk (no devices; edit marketing/app-stores/shots/shots.json for copy and shot changes)
	@cd marketing/app-stores/shots && bun install --frozen-lockfile && bunx playwright install chromium && bun compose.ts

# Store listings
STORES_DIR = $(CURDIR)/marketing/app-stores
# SDKROOT: gems with native extensions fail to link against the Command Line Tools SDK when it is newer than the selected Xcode.
STORES_FASTLANE = cd $(STORES_DIR) && export SDKROOT="$$(xcrun --sdk macosx --show-sdk-path)" BUNDLE_PATH="$(STORES_DIR)/vendor/bundle" LC_ALL=en_US.UTF-8 LANG=en_US.UTF-8 && bundle install --quiet && bundle exec fastlane

# Language folders directly under $(1), one per line (review_information is deliver's, not a language).
store_languages = for d in "$(1)"/*/; do [ -d "$$d" ] && n=$$(basename "$$d") && [ "$$n" != review_information ] && echo "$$n"; done
STORES_LIVE = $(STORES_DIR)/.live

.PHONY: store-pull-ios
store-pull-ios: ## Read the current App Store listing text into marketing/app-stores/.live/ios, replacing it (read-only; key=<App Store Connect API key JSON with key_id, issuer_id, key>). Review contact details and the demo login are deleted afterwards
	@test -n "$(key)" || { echo "$(RED)Error: pass key=/path/to/asc-key.json$(NC)"; exit 1; }
	@rm -rf "$(STORES_LIVE)/ios"; mkdir -p "$(STORES_LIVE)/ios"; \
	$(STORES_FASTLANE) deliver download_metadata --api_key_path "$(abspath $(key))" --app_identifier $(IOS_PROD_BUNDLE_ID) \
		--metadata_path "$(STORES_LIVE)/ios" --screenshots_path "$$(mktemp -d)" --force; \
	STATUS=$$?; rm -rf "$(STORES_LIVE)/ios/review_information"; \
	echo "App Store languages: $$($(call store_languages,$(STORES_LIVE)/ios) | tr '\n' ' ')"; exit $$STATUS

.PHONY: store-pull-play
store-pull-play: ## Read the current Google Play listing text and images into marketing/app-stores/.live/android, replacing it (read-only; needs gcloud application-default credentials for the Play service account)
	@rm -rf "$(STORES_LIVE)/android"; mkdir -p "$(STORES_LIVE)"; \
	$(STORES_FASTLANE) supply init --package_name com.sverto.app --metadata_path "$(STORES_LIVE)/android"; \
	STATUS=$$?; \
	echo "Google Play languages: $$($(call store_languages,$(STORES_LIVE)/android) | tr '\n' ' ')"; exit $$STATUS

.PHONY: store-push-play
store-push-play: ## Send the Google Play listing text and images (no bundle) through the release lane. Validate-only: Google checks the edit and nothing is published unless publish=1. Needs gcloud application-default credentials for the Play service account
	@unset PLAY_AAB PLAY_JSON_KEY; export PLAY_VALIDATE_ONLY=$(if $(filter 1,$(publish)),false,true); \
	$(STORES_FASTLANE) android release

.PHONY: store-push-ios
store-push-ios: ## Upload the App Store listing text and screenshots to the current editable version (key=<App Store Connect API key JSON with key_id, issuer_id, key>; duo=1 adds the Duo screenshots; needs APP_REVIEW_DEMO_USER and APP_REVIEW_DEMO_PASSWORD in the environment). No dry run: it changes App Store Connect straight away, and submits nothing
	@test -n "$(key)" || { echo "$(RED)Error: pass key=/path/to/asc-key.json$(NC)"; exit 1; }
	@unset IOS_APP_VERSION; export APP_STORE_CONNECT_KEY_JSON="$(abspath $(key))" IOS_UPLOAD_SCREENSHOTS=true IOS_INCLUDE_DUO=$(if $(filter 1,$(duo)),true,false); \
	$(STORES_FASTLANE) ios listing

STORE_LOCALE_ROOTS_ios = ios/metadata ios/screenshots ios/screenshots-duo
STORE_LOCALE_ROOTS_play = android/metadata android/foldable
# Plain `=` so only the command line sets these, not variables left in the environment.
STORE =
LOCALE =

.PHONY: store-locale
store-locale: ## Rename a store's language folder in its listing, screenshot and foldable folders: STORE=ios|play LOCALE=en-GB (plain mv; fails if that folder already exists)
	@case "$(STORE)" in ios|play) ;; *) echo "$(RED)Error: pass STORE=ios or STORE=play$(NC)"; exit 1 ;; esac; \
	test -n "$(LOCALE)" || { echo "$(RED)Error: pass LOCALE=<store language code, e.g. en-GB>$(NC)"; exit 1; }; \
	cd "$(STORES_DIR)" || exit 1; \
	METADATA=$(firstword $(STORE_LOCALE_ROOTS_$(STORE))); CURRENT=$$($(call store_languages,$$METADATA)); \
	[ -n "$$CURRENT" ] && [ "$$(echo "$$CURRENT" | wc -l)" -eq 1 ] || { echo "$(RED)Error: $$METADATA must hold exactly one language folder, found: $$(echo $$CURRENT)$(NC)"; exit 1; }; \
	for root in $(STORE_LOCALE_ROOTS_$(STORE)); do \
		[ ! -e "$$root/$(LOCALE)" ] || { echo "$(RED)Error: $$root/$(LOCALE) already exists$(NC)"; exit 1; }; \
	done; \
	for root in $(STORE_LOCALE_ROOTS_$(STORE)); do \
		if [ -d "$$root/$$CURRENT" ]; then mv "$$root/$$CURRENT" "$$root/$(LOCALE)" && echo "$$root/$$CURRENT -> $$root/$(LOCALE)" || exit 1; fi; \
	done

# API Generation
.PHONY: generate-api
generate-api: ## Generate TypeScript API client from OpenAPI spec
	@echo "$(GREEN)Compiling and generating OpenAPI spec...$(NC)"
	@TEMP_FILE=$$(mktemp /tmp/openapi.XXXXXX.json); \
	if (cd server && cargo run -p api --no-default-features --features database -- --openapi) > $$TEMP_FILE 2>/dev/null; then \
		echo "$(GREEN)OpenAPI spec generated successfully$(NC)"; \
		echo "$(YELLOW)Converting anyOf to oneOf...$(NC)"; \
		sed -i '' 's/"anyOf"/"oneOf"/g' $$TEMP_FILE; \
		echo "$(GREEN)Generating API client...$(NC)"; \
		cd web; \
		bunx @openapitools/openapi-generator-cli generate -i $$TEMP_FILE -g typescript-axios --skip-validate-spec -o src/api --additional-properties=withInterfaces=true,legacyDiscriminatorBehavior=true,supportsES6=true; \
		echo "$(YELLOW)Cleaning up generated files...$(NC)"; \
		rm -rf src/api/docs src/api/.openapi-generator src/api/.gitignore src/api/.npmignore src/api/.openapi-generator-ignore src/api/git_push.sh; \
		sed -i '' '/^\/\* eslint-disable \*\/$$/d' src/api/index.ts; \
		sed -i '' '1s/^/\/\/ @ts-nocheck\n/' src/api/api.ts; \
		echo "$(YELLOW)Removing temporary file...$(NC)"; \
		rm -f $$TEMP_FILE; \
		echo "$(YELLOW)Formatting generated files...$(NC)"; \
		bunx prettier --write "src/api/**/*.ts"; \
		echo "$(GREEN)API client generated successfully!$(NC)"; \
	else \
		echo "$(RED)Error: Failed to generate OpenAPI spec. Check Rust compilation errors:$(NC)"; \
		(cd server && cargo run -p api --no-default-features --features database -- --openapi) 2>&1 || true; \
		rm -f $$TEMP_FILE; \
		exit 1; \
	fi
