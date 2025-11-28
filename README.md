# nathanprice.dev

Static personal site for nathanprice.dev rendered with a custom Rust + [Tera](https://tera.netlify.app/) generator. Markdown content lives in `content/`, templates in `templates/`, and assets in `static/`.

## Prerequisites
- Rust toolchain (Cargo) installed.

## Local build
Generate the static site into `public/`:

```bash
cargo run --release
```

Then open `public/index.html` in your browser or serve the directory with any static file server.

## Development notes
- Update Markdown content in `content/` (front matter uses TOML with `+++` delimiters).
- HTML structure and styling live in `templates/` and `static/css/main.css`.
- Re-run `cargo run --release` after content or template changes to refresh the generated output.
