# Adapted from alswl/makefile-go sample.mk; see hack/makefile-go/README.md.
SHELL := /bin/bash
.DEFAULT_GOAL := build
export GOTOOLCHAIN := local
export CGO_ENABLED := 0

include hack/makefile-go/general.mk

.PHONY: check check-toolchain vet test test-raw verify-modules install generate-manual
ROOT := github.com/alswl/mind-forge
TARGETS := mf
CMD_DIR := ./cmd
OUTPUT_DIR := ./bin
GOOS ?= $(shell go env GOOS)
GOARCH ?= $(shell go env GOARCH)
GO_BUILD_VERBOSE :=
GO_SOURCE_DIRS := cmd pkg internal
GO_LINT_COMMAND := bash scripts/lint.sh
MF_BUILD_OUT ?= $(OUTPUT_DIR)/mf
GO_BUILD_OUTPUT = $(MF_BUILD_OUT)
GO_BUILD_COPY_VARIANTS := false
COMMIT := $(shell git rev-parse --short HEAD)
BUILD_DATE := $(shell date -u +%Y-%m-%d)
GO_BUILD_LDFLAGS := -X $(ROOT)/internal/version.Commit=$(COMMIT) -X $(ROOT)/internal/version.BuildDate=$(BUILD_DATE)
export GIT_DIR := $(shell git rev-parse --absolute-git-dir)
export GIT_WORK_TREE := $(shell git rev-parse --show-toplevel)

include hack/makefile-go/build.mk

build: check-toolchain
check-toolchain:
	@expected_go="go$$(awk '$$1 == "golang" { print $$2 }' .tool-versions)"; \
	  [[ "$$(go env GOVERSION)" == "$$expected_go" ]] || { printf 'Expected %s; select the version in .tool-versions.\n' "$$expected_go" >&2; exit 1; }

check: ## Run the complete local quality gate.
	bash scripts/check.sh

vet: ## Run Go static analysis.
	go vet ./...

test: ## Run colocated component tests.
	go test -count=1 ./cmd/... ./pkg/... ./internal/...

test-raw: test ## Alias for component tests without coverage.

verify-modules: ## Verify module checksums and tidy state without changing files.
	go mod verify
	go mod tidy -diff

install: ## Install from source using the existing installer.
	bash scripts/install.sh

generate-manual: ## Regenerate the CLI reference manual.
	bash scripts/generate-manual.sh
