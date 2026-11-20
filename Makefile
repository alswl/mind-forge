# Adapted from alswl/makefile-go sample.mk; see hack/makefile-go/README.md.
SHELL := /bin/bash
.DEFAULT_GOAL := build
export GOTOOLCHAIN := local
export CGO_ENABLED := 0

MAKEFILE_GO := hack/.upstream-makefile-go/hack
ifeq ($(wildcard $(MAKEFILE_GO)/makefile-go/general.mk),)
$(error Missing makefile-go submodule; run git submodule update --init --recursive)
endif
include $(MAKEFILE_GO)/makefile-go/general.mk

.PHONY: check check-toolchain check-linter vet test test-raw verify-modules install generate-manual
ROOT := github.com/alswl/mind-forge
TARGETS := mf
CMD_DIR := ./cmd
OUTPUT_DIR := ./bin
GOOS ?= $(shell go env GOOS)
GOARCH ?= $(shell go env GOARCH)
GO_BUILD_VERBOSE :=
GO_SOURCE_DIRS := cmd pkg internal
GO_LINT_READ_ONLY := true
GO_LINT_COMMAND := golangci-lint run ./...
MF_BUILD_OUT ?= $(OUTPUT_DIR)/mf
GO_BUILD_OUTPUT = $(MF_BUILD_OUT)
GO_BUILD_COPY_VARIANTS := false
COMMIT := $(shell git rev-parse --short HEAD)
BUILD_DATE := $(shell date -u +%Y-%m-%d)
GO_BUILD_LDFLAGS := -X $(ROOT)/internal/version.Commit=$(COMMIT) -X $(ROOT)/internal/version.BuildDate=$(BUILD_DATE)
export GIT_DIR := $(shell git rev-parse --absolute-git-dir)
export GIT_WORK_TREE := $(shell git rev-parse --show-toplevel)

override BUMP_MODE := prepare
BUMP_SCRIPT := $(MAKEFILE_GO)/bump.sh
BUMP_PREPARE_FILES := internal/version/version.go
DEFAULT_BUMP_STAGE := final
DEFAULT_BUMP_SCOPE := patch
include $(MAKEFILE_GO)/makefile-go/build.mk
include $(MAKEFILE_GO)/makefile-go/version.mk

bump bump-patch bump-minor bump-major bump-beta bump-alpha: check-toolchain

build: check-toolchain
check-toolchain:
	@bash hack/check-version.sh
	@expected_go="go$$(awk '$$1 == "golang" { print $$2 }' .tool-versions)"; \
	  [[ "$$(go env GOVERSION)" == "$$expected_go" ]] || { printf 'Expected %s; select the version in .tool-versions.\n' "$$expected_go" >&2; exit 1; }

lint: check-linter
check-linter:
	@expected=2.12.2; \
	  command -v golangci-lint >/dev/null || { printf 'Install golangci-lint v%s to run lint.\n' "$$expected" >&2; exit 1; }; \
	  actual=$$(golangci-lint version --short) && \
	  [[ "$$actual" == "$$expected" ]] || { printf 'Expected golangci-lint %s; found %s.\n' "$$expected" "$$actual" >&2; exit 1; }

check: check-toolchain ## Run the complete local quality gate.
	$(MAKE) --no-print-directory lint
	$(MAKE) --no-print-directory vet
	go mod verify
	$(MAKE) --no-print-directory test
	$(MAKE) --no-print-directory build

vet: ## Run Go static analysis.
	go vet ./...

test: ## Run colocated component tests.
	go test -count=1 ./cmd/... ./pkg/... ./internal/...

test-raw: test ## Alias for component tests without coverage.

verify-modules: ## Verify module checksums and tidy state without changing files.
	go mod verify
	go mod tidy -diff

install: ## Install from source using the existing installer.
	bash hack/install.sh

generate-manual: ## Regenerate the CLI reference manual.
	bash hack/generate-manual.sh
