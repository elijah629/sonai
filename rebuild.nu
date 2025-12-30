cd training-bin
cargo r -r
cd ../sonai
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen ../target/wasm32-unknown-unknown/release/sonai.wasm --out-dir ../inference-wasm-web/src/pkg --target bundler
# wasm-opt -O4 --strip-debug --enable-bulk-memory-opt -o ../inference-wasm-web/src/pkg/sonai_bg.wasm ../inference-wasm-web/src/pkg/sonai_bg.wasm
cd ../inference-wasm-web
bun run dev
