## 🚴 Usage

### 🛠️ Build and serve:

```
./build build
./build serve
```

```
./build watch
```

will run cargo watch and build the wasm file.

### 📖 Generate documentation with:

```
./build docs
```

This runs `cargo doc --document-private-items --open`.

### 🔬 Test in Headless Browsers with `wasm-pack test`

```
wasm-pack test --headless --firefox
```
