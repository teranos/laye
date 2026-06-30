WASM_DIST := dist

.PHONY: wasm clean

wasm:
	cargo build --target wasm32-unknown-unknown --release --lib --package bevy-starter
	wasm-bindgen ../target/wasm32-unknown-unknown/release/bevy_starter.wasm --target web --out-dir $(WASM_DIST) --no-typescript
	cp web/index.html $(WASM_DIST)/

clean:
	rm -rf $(WASM_DIST)
