CARGO_PROFILE ?= release

test:
	@for file in tests/*.json; do \
		name=$$(basename "$$file" .json); \
		rm -rf "tests/$$name"; \
		cargo run --release --bin solores -- --cargo-edition 2021 --output-dir tests/ --output-crate-name "$$name" "$$file" --solana-program-vers "^2.1" --borsh-vers "^1.5" --thiserror-vers "^1.0" --num-derive-vers "0.4.2" --num-traits-vers "^0.2" --serde-vers "^1" --serde-bytes-vers "0.11.19" --serde-big-array-vers "0.5" --bytemuck-vers "^1.16" --pinocchio-vers "0.10.2"; \
		test -f "tests/$$name/Cargo.toml" || (echo "ERROR: tests/$$name/Cargo.toml not found" && exit 1); \
	done

.PHONY: all test
