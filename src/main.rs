use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use tera::{Context as TeraContext, Tera};
use walkdir::WalkDir;

// Configuration paths
const CONFIG_PATH: &str = "site.toml";
const CONTENT_DIR: &str = "content";
const TEMPLATES_GLOB: &str = "templates/**/*";
const STATIC_DIR: &str = "static";
const OUTPUT_DIR: &str = "public";

#[derive(Debug, Deserialize, Serialize)]
struct Config {
    base_url: String,
    title: String,
    description: String,
    #[serde(default)]
    extra: HashMap<String, toml::Value>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct FrontMatter {
    title: Option<String>,
    description: Option<String>,
    template: Option<String>,
    date: Option<NaiveDate>,
    summary: Option<String>,
    /// Reserved for future use - will support sorting by date, title, etc.
    #[allow(dead_code)]
    sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PageData {
    title: String,
    date: Option<NaiveDate>,
    summary: Option<String>,
    content: String,
    permalink: String,
    relative_path: String,
    template: Option<String>,
    slug: String,
}

#[derive(Debug, Clone)]
struct SectionContent {
    meta: FrontMatter,
    body_html: String,
    pages: Vec<PageData>,
}

#[derive(Debug, Clone, Serialize)]
struct SectionData {
    title: String,
    description: Option<String>,
    pages: Vec<PageData>,
    content: String,
}

fn main() -> Result<()> {
    build_site()
}

fn build_site() -> Result<()> {
    let config = load_config(CONFIG_PATH)?;
    let tera = Tera::new(TEMPLATES_GLOB).context("loading templates")?;

    let content_dir = Path::new(CONTENT_DIR);
    let output_dir = Path::new(OUTPUT_DIR);

    if output_dir.exists() {
        fs::remove_dir_all(output_dir).context("clearing public directory")?;
    }
    fs::create_dir_all(output_dir).context("creating public directory")?;

    copy_static_assets(Path::new(STATIC_DIR), output_dir)?;

    let (root_section, sections) = load_content(content_dir, &config.base_url)?;

    // Validate and warn about potential issues
    validate_content(&sections);

    render_home(&tera, &config, &sections, output_dir, &root_section)?;
    render_sections(&tera, &config, &sections, output_dir)?;
    render_pages(&tera, &config, &sections, output_dir)?;
    render_404(&tera, &config, output_dir)?;

    Ok(())
}

fn load_config(path: &str) -> Result<Config> {
    let contents = fs::read_to_string(path).context("reading site.toml")?;
    let mut config: Config = toml::from_str(&contents).context("parsing site.toml")?;
    config.base_url = config.base_url.trim_end_matches('/').to_string();
    Ok(config)
}

fn copy_static_assets(static_dir: &Path, output_dir: &Path) -> Result<()> {
    if !static_dir.exists() {
        return Ok(());
    }

    for entry in WalkDir::new(static_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let relative = path.strip_prefix(static_dir).unwrap();
            let dest = output_dir.join(relative);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, dest)?;
        }
    }

    Ok(())
}

fn parse_front_matter(content: &str) -> Result<(FrontMatter, String)> {
    let mut lines = content.lines();
    let first_line = lines.next().unwrap_or("");

    if first_line.trim() != "+++" {
        return Ok((FrontMatter::default(), content.to_string()));
    }

    let mut front_matter = String::new();
    let mut body = String::new();
    let mut in_front_matter = true;

    for line in lines {
        if in_front_matter && line.trim() == "+++" {
            in_front_matter = false;
            continue;
        }

        if in_front_matter {
            front_matter.push_str(line);
            front_matter.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }

    let data: FrontMatter = toml::from_str(&front_matter)
        .context("parsing frontmatter TOML")?;
    Ok((data, body))
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(markdown, options);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    html_output
}

/// Renders a template with the given context and writes to output file
fn render_template_to_file(
    tera: &Tera,
    template_name: &str,
    context: &TeraContext,
    output_path: &Path,
    context_desc: &str,
) -> Result<()> {
    let rendered = tera
        .render(template_name, context)
        .with_context(|| format!("rendering {}", context_desc))?;

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating directory for {}", context_desc))?;
    }

    fs::write(output_path, rendered)
        .with_context(|| format!("writing {} to {:?}", context_desc, output_path))?;

    Ok(())
}

/// Creates base template context with config and path prefix
fn build_base_context(config: &Config, path_prefix: &str) -> TeraContext {
    let mut context = TeraContext::new();
    context.insert("config", config);
    context.insert("path_prefix", path_prefix);
    context
}

/// Calculates directory depth for path prefix generation
/// Returns number of "../" needed to reach site root
fn calculate_path_depth(path: &str, is_page: bool) -> usize {
    if path.is_empty() {
        if is_page { 1 } else { 0 }
    } else {
        let base_depth = path.split('/').count();
        if is_page { base_depth + 1 } else { base_depth }
    }
}

/// Generates "../" prefix to reach site root from given depth
fn path_prefix_for_depth(depth: usize) -> String {
    "../".repeat(depth)
}

fn load_content(
    content_dir: &Path,
    base_url: &str,
) -> Result<(SectionData, HashMap<String, SectionContent>)> {
    let mut sections: HashMap<String, SectionContent> = HashMap::new();
    let mut root_meta = FrontMatter::default();
    let mut root_body = String::new();

    for entry in WalkDir::new(content_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file() && e.path().extension().map(|e| e == "md").unwrap_or(false))
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(content_dir)
            .context("stripping content prefix")?;
        let parent = relative
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
        let parent_key = parent.to_string_lossy().to_string();

        let raw = fs::read_to_string(path)
            .with_context(|| format!("reading markdown file {:?}", path))?;
        let (meta, body) = parse_front_matter(&raw)
            .with_context(|| format!("parsing frontmatter in {:?}", path))?;
        let html_body = markdown_to_html(&body);

        if path.file_name().unwrap() == "_index.md" {
            if relative.components().count() == 1 {
                root_meta = meta;
                root_body = html_body;
            } else {
                // Use entry API to preserve existing pages if section already exists
                sections.entry(parent_key.clone())
                    .and_modify(|section| {
                        section.meta = meta.clone();
                        section.body_html = html_body.clone();
                    })
                    .or_insert_with(|| SectionContent {
                        meta,
                        body_html: html_body,
                        pages: Vec::new(),
                    });
            }
            continue;
        }

        let slug = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("page")
            .to_string();

        let mut url_path = PathBuf::new();
        if !parent_key.is_empty() {
            url_path.push(&parent_key);
        }
        url_path.push(&slug);

        let url_str = url_path.to_string_lossy();
        let permalink = format!("{}/{}/", base_url, url_str);
        let relative_path = if parent_key.is_empty() {
            format!("{}/index.html", slug)
        } else {
            format!("{}/{}/index.html", parent_key, slug)
        };

        let page = PageData {
            title: meta
                .title
                .clone()
                .unwrap_or_else(|| slug.replace('-', " ").to_uppercase()),
            date: meta.date,
            summary: meta.summary.clone(),
            content: html_body,
            permalink,
            relative_path,
            template: meta.template.clone(),
            slug,
        };

        sections
            .entry(parent_key.clone())
            .or_insert_with(|| SectionContent {
                meta: FrontMatter::default(),
                body_html: String::new(),
                pages: Vec::new(),
            })
            .pages
            .push(page);
    }

    for (_, section) in sections.iter_mut() {
        section.pages.sort_by(|a, b| b.date.cmp(&a.date));
    }

    let root_section = SectionData {
        title: root_meta.title.unwrap_or_else(|| "Home".to_string()),
        description: root_meta.description,
        pages: Vec::new(),
        content: root_body,
    };

    Ok((root_section, sections))
}

/// Validates loaded content and prints warnings for common issues
fn validate_content(sections: &HashMap<String, SectionContent>) {
    let mut seen_slugs: HashMap<String, Vec<String>> = HashMap::new();

    for (section_key, section) in sections {
        // Check for missing titles in section metadata
        if section.meta.title.is_none() {
            eprintln!("⚠️  Warning: Section '{}' has no title", section_key);
        }

        // Check for duplicate slugs within sections
        for page in &section.pages {
            let key = format!("{}/{}", section_key, &page.slug);
            seen_slugs.entry(page.slug.clone())
                .or_insert_with(Vec::new)
                .push(key);
        }

        // Check for pages without dates (affects sorting)
        let undated: Vec<_> = section.pages.iter()
            .filter(|p| p.date.is_none())
            .collect();
        if !undated.is_empty() && !section.pages.is_empty() {
            eprintln!("⚠️  Warning: Section '{}' has {} pages without dates (may affect sorting)",
                section_key, undated.len());
        }
    }

    // Report duplicate slugs
    for (slug, locations) in seen_slugs {
        if locations.len() > 1 {
            eprintln!("⚠️  Warning: Duplicate slug '{}' found in: {}",
                slug, locations.join(", "));
        }
    }
}

fn render_home(
    tera: &Tera,
    config: &Config,
    sections: &HashMap<String, SectionContent>,
    output_dir: &Path,
    root_section: &SectionData,
) -> Result<()> {
    let mut context = build_base_context(config, "");
    context.insert("section", root_section);

    if let Some(section) = sections.get("writing") {
        context.insert("writing_pages", &section.pages);
    } else {
        context.insert("writing_pages", &Vec::<PageData>::new());
    }

    render_template_to_file(
        tera,
        "index.html",
        &context,
        &output_dir.join("index.html"),
        "homepage",
    )
}

fn render_sections(
    tera: &Tera,
    config: &Config,
    sections: &HashMap<String, SectionContent>,
    output_dir: &Path,
) -> Result<()> {
    for (key, section_content) in sections.iter() {
        let template = section_content
            .meta
            .template
            .clone()
            .unwrap_or_else(|| "section.html".to_string());

        let mut dest_dir = output_dir.to_path_buf();
        if !key.is_empty() {
            dest_dir.push(key);
        }

        let depth = calculate_path_depth(key, false);
        let path_prefix = path_prefix_for_depth(depth);

        if template == "page.html" {
            let page = PageData {
                title: section_content
                    .meta
                    .title
                    .clone()
                    .unwrap_or_else(|| key.clone()),
                date: section_content.meta.date,
                summary: section_content.meta.summary.clone(),
                content: section_content.body_html.clone(),
                permalink: format!("{}/{}/", config.base_url, key),
                relative_path: format!("{}/index.html", key),
                template: section_content.meta.template.clone(),
                slug: key.clone(),
            };

            let mut context = build_base_context(config, &path_prefix);
            context.insert("page", &page);

            render_template_to_file(
                tera,
                &template,
                &context,
                &dest_dir.join("index.html"),
                &format!("section page {}", key),
            )?;
            continue;
        }

        let section = SectionData {
            title: section_content
                .meta
                .title
                .clone()
                .unwrap_or_else(|| key.clone()),
            description: section_content.meta.description.clone(),
            pages: section_content.pages.clone(),
            content: section_content.body_html.clone(),
        };

        let mut context = build_base_context(config, &path_prefix);
        context.insert("section", &section);

        render_template_to_file(
            tera,
            &template,
            &context,
            &dest_dir.join("index.html"),
            &format!("section {}", key),
        )?;
    }

    Ok(())
}

fn render_pages(
    tera: &Tera,
    config: &Config,
    sections: &HashMap<String, SectionContent>,
    output_dir: &Path,
) -> Result<()> {
    for (key, section) in sections.iter() {
        for page in &section.pages {
            let page_template = page
                .template
                .clone()
                .or_else(|| section.meta.template.clone())
                .unwrap_or_else(|| "page.html".to_string());

            let depth = calculate_path_depth(key, true);
            let path_prefix = path_prefix_for_depth(depth);

            let mut context = build_base_context(config, &path_prefix);
            context.insert("page", page);

            let mut dest_dir = output_dir.to_path_buf();
            if !key.is_empty() {
                dest_dir.push(key);
            }
            dest_dir.push(&page.slug);

            render_template_to_file(
                tera,
                &page_template,
                &context,
                &dest_dir.join("index.html"),
                &format!("page {}", page.title),
            )?;
        }
    }

    Ok(())
}

fn render_404(tera: &Tera, config: &Config, output_dir: &Path) -> Result<()> {
    let context = build_base_context(config, "");

    render_template_to_file(
        tera,
        "404.html",
        &context,
        &output_dir.join("404.html"),
        "404 page",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_front_matter_with_all_fields() {
        let input = r#"+++
title = "Test Post"
description = "A test"
date = "2025-01-15"
summary = "Summary here"
template = "custom.html"
sort_by = "date"
+++
Content here"#;

        let result = parse_front_matter(input);
        assert!(result.is_ok());
        let (fm, body) = result.unwrap();

        assert_eq!(fm.title, Some("Test Post".to_string()));
        assert_eq!(fm.description, Some("A test".to_string()));
        assert_eq!(fm.date, Some(NaiveDate::from_ymd_opt(2025, 1, 15).unwrap()));
        assert_eq!(fm.summary, Some("Summary here".to_string()));
        assert_eq!(fm.template, Some("custom.html".to_string()));
        assert_eq!(body.trim(), "Content here");
    }

    #[test]
    fn test_parse_front_matter_minimal() {
        let input = r#"+++
title = "Minimal"
+++
Body"#;

        let result = parse_front_matter(input);
        assert!(result.is_ok());
        let (fm, body) = result.unwrap();

        assert_eq!(fm.title, Some("Minimal".to_string()));
        assert_eq!(fm.description, None);
        assert_eq!(body.trim(), "Body");
    }

    #[test]
    fn test_parse_front_matter_no_frontmatter() {
        let input = "Just content, no frontmatter";

        let result = parse_front_matter(input);
        assert!(result.is_ok());
        let (fm, body) = result.unwrap();

        assert_eq!(fm.title, None);
        assert_eq!(body, input);
    }

    #[test]
    fn test_markdown_to_html_basic() {
        let md = "# Heading\n\nParagraph with **bold**";
        let html = markdown_to_html(md);

        assert!(html.contains("<h1>"));
        assert!(html.contains("<strong>"));
        assert!(html.contains("Heading"));
    }

    #[test]
    fn test_path_depth_calculation() {
        assert_eq!(calculate_path_depth("", false), 0);
        assert_eq!(calculate_path_depth("", true), 1);
        assert_eq!(calculate_path_depth("section", false), 1);
        assert_eq!(calculate_path_depth("section", true), 2);
        assert_eq!(calculate_path_depth("section/nested", false), 2);
        assert_eq!(calculate_path_depth("section/nested", true), 3);
    }

    #[test]
    fn test_path_prefix_generation() {
        assert_eq!(path_prefix_for_depth(0), "");
        assert_eq!(path_prefix_for_depth(1), "../");
        assert_eq!(path_prefix_for_depth(2), "../../");
        assert_eq!(path_prefix_for_depth(3), "../../../");
    }

    #[test]
    fn test_relative_path_generation_root() {
        let parent_key = "";
        let slug = "about";

        let relative_path = if parent_key.is_empty() {
            format!("{}/index.html", slug)
        } else {
            format!("{}/{}/index.html", parent_key, slug)
        };

        assert_eq!(relative_path, "about/index.html");
        assert!(!relative_path.starts_with('/'));
    }

    #[test]
    fn test_relative_path_generation_nested() {
        let parent_key = "writing";
        let slug = "my-post";

        let relative_path = if parent_key.is_empty() {
            format!("{}/index.html", slug)
        } else {
            format!("{}/{}/index.html", parent_key, slug)
        };

        assert_eq!(relative_path, "writing/my-post/index.html");
    }
}
