# MindForge compatibility hooks; upstream origin and patch are documented in README.md.
GO_BUILD_VERBOSE ?= -v
GO_SOURCE_DIRS ?= .
GO_LINT_COMMAND ?= golangci-lint run --timeout 5m
GO_BUILD_LDFLAGS ?= -s -w -X $(ROOT)/pkg/version.Version=$(VERSION) -X $(ROOT)/pkg/version.Commit=$(COMMIT) -X $(ROOT)/pkg/version.Package=$(ROOT) -X $(ROOT)/internal/version.Version=$(VERSION) -X $(ROOT)/internal/version.Commit=$(COMMIT) -X $(ROOT)/internal/version.Package=$(ROOT)
GO_BUILD_OUTPUT ?= $(OUTPUT_DIR)/$${target}-$(GOOS)-$(GOARCH)
GO_BUILD_COPY_VARIANTS ?= true

.PHONY: download
download: ## Run go mod download
	go mod download


.PHONY: fmt
fmt: ## Run format code
	gofmt -w $(GO_SOURCE_DIRS)
	go fmt ./...
	# vet is run by golangci-lint
	# https://golangci-lint.run/usage/linters/
	# go vet ./...
	goimports -w $(GO_SOURCE_DIRS)
	golangci-lint run --fix


.PHONY: lint
lint: ## Run lint
	@echo "# gofmt"
	@test $$(gofmt -l $(GO_SOURCE_DIRS) | wc -l) -eq 0

	@echo "# ensure integration test with // +build integration"
	@if [ -n "$$(find . -name '*_integration_test.go')" ]; then                                                                                                  \
		test $$(find . -name '*_integration_test.go' | wc -l) -eq $$(cat $$(find . -name '*_integration_test.go') | grep -E '// ?\+build integration' | wc -l);  \
	fi

	go mod tidy -diff
	$(GO_LINT_COMMAND)

	# (cd examples && go mod tidy && golangci-lint run --timeout 5m && gofmt -w $(GO_SOURCE_DIRS))


.PHONY: clean
clean: ## Clean temp files
	@rm -vrf ${OUTPUT_DIR}/*


.PHONY: build
build: ## Build
	@for target in $(TARGETS); do \
	  mkdir -p "$$(dirname "$(GO_BUILD_OUTPUT)")" || exit; \
	  GOOS=$(GOOS) GOARCH=$(GOARCH) go build $(GO_BUILD_VERBOSE) -o "$(GO_BUILD_OUTPUT)" \
	    $(if $(GO_BUILD_TAGS),-tags=$(GO_BUILD_TAGS),) \
	    -ldflags "$(GO_BUILD_LDFLAGS)" $(CMD_DIR)/$${target} || exit; \
	  if [[ "$(GO_BUILD_COPY_VARIANTS)" == true ]]; then \
	    cp "$(GO_BUILD_OUTPUT)" "$(OUTPUT_DIR)/$${target}" || exit; \
	    cp "$(GO_BUILD_OUTPUT)" "$(OUTPUT_DIR)/$${target}-$(GOOS)-$(GOARCH)-$(VERSION)" || exit; \
	    cp "$(GO_BUILD_OUTPUT)" "$(OUTPUT_DIR)/$${target}-$(VERSION)" || exit; \
	  fi; \
	done


.PHONY: compress
compress: ## Compress by upx
	@# move out of loop to avoid syntax error
	@# upx not works in debian for security reason
	@# upx $(OUTPUT_DIR)/$${target};
	@for target in $(TARGETS); do                                                                      \
	    echo "";                                                                                       \
	done
