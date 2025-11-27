use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::NaiveDate;
use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use tera::{Context as TeraContext, Tera};
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
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
    sort_by: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PageData {
    title: String,
    date: Option<NaiveDate>,
    summary: Option<String>,
    content: String,
    permalink: String,
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
    let config = load_config("config.toml")?;
    let tera = Tera::new("templates/**/*").context("loading templates")?;

    let content_dir = Path::new("content");
    let output_dir = Path::new("public");

    if output_dir.exists() {
        fs::remove_dir_all(output_dir).context("clearing public directory")?;
    }
    fs::create_dir_all(output_dir).context("creating public directory")?;

    copy_static_assets(Path::new("static"), output_dir)?;

    let (root_section, sections) = load_content(content_dir, &config.base_url)?;

    render_home(&tera, &config, &sections, output_dir, &root_section)?;
    render_sections(&tera, &config, &sections, output_dir)?;
    render_pages(&tera, &config, &sections, output_dir)?;
    render_404(&tera, &config, output_dir)?;

    Ok(())
}

fn load_config(path: &str) -> Result<Config> {
    let contents = fs::read_to_string(path).context("reading config.toml")?;
    let mut config: Config = toml::from_str(&contents).context("parsing config.toml")?;
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

    let data: FrontMatter = toml::from_str(&front_matter).context("parsing front matter")?;
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

        let raw = fs::read_to_string(path).context("reading markdown file")?;
        let (meta, body) = parse_front_matter(&raw)?;
        let html_body = markdown_to_html(&body);

        if path.file_name().unwrap() == "_index.md" {
            if relative.components().count() == 1 {
                root_meta = meta;
                root_body = html_body;
            } else {
                sections.insert(
                    parent_key.clone(),
                    SectionContent {
                        meta,
                        body_html: html_body,
                        pages: Vec::new(),
                    },
                );
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

        let page = PageData {
            title: meta
                .title
                .clone()
                .unwrap_or_else(|| slug.replace('-', " ").to_uppercase()),
            date: meta.date,
            summary: meta.summary.clone(),
            content: html_body,
            permalink,
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

fn render_home(
    tera: &Tera,
    config: &Config,
    sections: &HashMap<String, SectionContent>,
    output_dir: &Path,
    root_section: &SectionData,
) -> Result<()> {
    let mut context = TeraContext::new();
    context.insert("config", config);
    context.insert("section", root_section);

    if let Some(section) = sections.get("writing") {
        context.insert("writing_pages", &section.pages);
    } else {
        context.insert("writing_pages", &Vec::<PageData>::new());
    }

    let rendered = tera
        .render("index.html", &context)
        .context("rendering homepage")?;

    fs::write(output_dir.join("index.html"), rendered).context("writing homepage")?;
    Ok(())
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

        fs::create_dir_all(&dest_dir).context("creating section directory")?;

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
                template: section_content.meta.template.clone(),
                slug: key.clone(),
            };

            let mut context = TeraContext::new();
            context.insert("config", config);
            context.insert("page", &page);

            let rendered = tera
                .render(&template, &context)
                .with_context(|| format!("rendering section page {key}"))?;

            fs::write(dest_dir.join("index.html"), rendered)?;
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

        let mut context = TeraContext::new();
        context.insert("config", config);
        context.insert("section", &section);

        let rendered = tera
            .render(&template, &context)
            .with_context(|| format!("rendering section {key}"))?;

        fs::write(dest_dir.join("index.html"), rendered)?;
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

            let mut context = TeraContext::new();
            context.insert("config", config);
            context.insert("page", page);

            let rendered = tera
                .render(&page_template, &context)
                .with_context(|| format!("rendering page {}", page.title))?;

            let mut dest_dir = output_dir.to_path_buf();
            if !key.is_empty() {
                dest_dir.push(key);
            }

            dest_dir.push(&page.slug);
            fs::create_dir_all(&dest_dir).context("creating page directory")?;
            fs::write(dest_dir.join("index.html"), rendered)?;
        }
    }

    Ok(())
}

fn render_404(tera: &Tera, config: &Config, output_dir: &Path) -> Result<()> {
    let mut context = TeraContext::new();
    context.insert("config", config);

    let rendered = tera
        .render("404.html", &context)
        .context("rendering 404 page")?;
    fs::write(output_dir.join("404.html"), rendered)?;
    Ok(())
}
