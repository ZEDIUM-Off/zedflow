CARGO ?= cargo
PACKAGE := zedflow-agent
HARNESS_TESTS := \
	--test agent-harness \
	--test agent-harness-stream \
	--test harness \
	--test nodejs-env \
	--test prompt-templates \
	--test resource-formatting \
	--test skills \
	--test system-prompt \
	--test truncate

.PHONY: all clean build test test\:harness coverage\:harness prepublishOnly fmt check doc package

all: build

clean:
	$(CARGO) clean -p $(PACKAGE)

build:
	$(CARGO) build -p $(PACKAGE)

test:
	$(CARGO) test -p $(PACKAGE) --all-targets

test\:harness:
	$(CARGO) test -p $(PACKAGE) $(HARNESS_TESTS)

coverage\:harness:
	@command -v cargo-llvm-cov >/dev/null 2>&1 || { echo "cargo llvm-cov is required; install it with: cargo install cargo-llvm-cov" >&2; exit 1; }
	$(CARGO) llvm-cov clean --workspace
	$(CARGO) llvm-cov -p $(PACKAGE) $(HARNESS_TESTS) --no-report
	$(CARGO) llvm-cov report --text
	@mkdir -p coverage/harness
	$(CARGO) llvm-cov report --html --output-dir coverage/harness/html
	$(CARGO) llvm-cov report --lcov --output-path coverage/harness/lcov.info

prepublishOnly: clean build

fmt:
	$(CARGO) fmt --package $(PACKAGE) --check

check:
	$(CARGO) check -p $(PACKAGE) --all-targets

doc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc -p $(PACKAGE) --no-deps

package:
	$(CARGO) package -p $(PACKAGE) --allow-dirty --list
