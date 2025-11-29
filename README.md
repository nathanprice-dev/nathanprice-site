# nathanprice.dev static site

This repository contains a small static-site generator written in Rust plus the Markdown content, templates, and assets that power [nathanprice.dev](https://nathanprice.dev). Running the binary reads the content in `content/`, renders it with [Tera](https://tera.netlify.app/) templates from `templates/`, copies assets from `static/`, and emits a complete site into `public/`.

## Getting started

1. Install the Rust toolchain (Cargo).
2. Build the site locally:

   ```bash
   cargo run --release
   ```

   The generated HTML lives in `public/`. Open `public/index.html` directly or serve the directory with any static file server.

## Authoring content

Content is organized by section under `content/`:

- Each section has an optional `_index.md` to provide metadata and body copy for the section landing page (e.g., `content/about/_index.md`).
- Individual posts or pages live alongside their section index (e.g., `content/writing/*.md`). The output slug matches the filename.
- Front matter uses TOML delimited by `+++`. Common fields include `title`, `description`, `date`, `summary`, and an optional `template` override.

Example post:

```markdown
+++
title = "My Post"
date = "2025-01-15"
summary = "One-line description"
template = "page.html" # optional override; falls back to section/page defaults
+++

Markdown body starts here.
```

After adding or editing content, re-run `cargo run --release` to regenerate `public/`.

## Templates, assets, and output

- Templates live in `templates/` and are loaded with the glob `templates/**/*`. Use `page.html` for individual pages and `section.html` for section listings; templates can be overridden per page or section via front matter.
- Static files in `static/` are copied verbatim into `public/` before rendering.
- The render target is always `public/`, which is fully cleared before each build to avoid stale files.

## Renderer architecture

The generator is implemented in `src/main.rs`:

- Configuration: `site.toml` is parsed into a `Config` struct that supplies the base URL, site metadata, and extra fields. Paths are normalized to avoid trailing slashes.
- Content loading: Markdown files are walked with `walkdir`, front matter is parsed as TOML, Markdown is rendered to HTML via `pulldown-cmark`, and section/page data is collected into in-memory structs. Section pages are sorted by date when present.
- Rendering pipeline: static assets are copied first, then the homepage, sections, and individual pages are rendered with Tera contexts that include the site config, the current entity (page or section), and a computed `path_prefix` for relative links. A 404 page is also emitted.
- Validation: during builds the loader warns about common authoring issues such as missing titles, duplicate slugs, and undated pages that may sort unexpectedly.

## Deployment

Automated deployment is handled by GitHub Actions and Cloudflare Pages:

- The `publish-deploy` workflow runs on pushes to `main` (or manually) and executes `cargo run --release` to generate `public/`.
- The workflow copies the rendered site into a fresh `publish/` directory, initializes a temporary git repo, and force-pushes the contents to the `publish` branch using the GitHub token.
- Cloudflare Pages is configured to read from the `publish` branch; once the workflow finishes, Cloudflare picks up the updated branch and publishes the site.

For local validation before pushing, build the site and inspect `public/` or host it with a static file server to ensure templates and links resolve correctly.
