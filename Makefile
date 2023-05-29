dev: clean sample_plugin clippy tests build install

clippy:
	@echo "Running clippy..."
	cargo clippy

sample_plugin: sample_plugin/src/lib.rs sample_plugin/Cargo.toml
	@echo "Building sample plugin..."
	cd sample_plugin && maturin build --release -o dist

build:
	@echo "Building..."
	cd savant && maturin build --release -o dist

install:
	@echo "Installing..."
	cd savant && pip3.10 install --force-reinstall dist/*.whl

clean:
	@echo "Cleaning..."
	cd savant && rm -rf dist/*.whl

tests: sample_plugin
	@echo "Running tests..."
	cd savant && cargo test --no-default-features  -- --nocapture

bench: sample_plugin
	@echo "Running benchmarks..."
	cd savant && cargo bench --no-default-features -- --nocapture