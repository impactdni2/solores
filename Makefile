CARGO_PROFILE ?= release

test:
	@for file in tests/*.json; do \
		name=$$(basename "$$file" .json); \
		rm -rf "tests/$$name"; \
		cargo run --release --bin solores -- --cargo-edition 2021 --output-crate-name "tests/$$name" "$$file"; \
		test -f "tests/$$name/Cargo.toml" || (echo "ERROR: tests/$$name/Cargo.toml not found" && exit 1); \
	done

.PHONY: all test
