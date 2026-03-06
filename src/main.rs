use mdbook_tikz::{compile_tikz, detect_tex_engine, wrap_svg_html, wrap_tikz_latex};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{self, Read as _, Write as _};
use std::path::PathBuf;
use std::process;

// ── mdBook protocol types ──

#[derive(Deserialize)]
struct Context {
    root: PathBuf,
    config: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct Book {
    #[serde(alias = "sections")]
    items: Vec<BookItem>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
#[allow(non_snake_case)]
enum BookItem {
    Chapter { Chapter: Chapter },
    Other(serde_json::Value),
}

#[derive(Serialize, Deserialize)]
struct Chapter {
    name: String,
    content: String,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

// ── Configuration ──

struct Config {
    preamble: String,
    tex_command: String,
    pdf2svg_command: String,
    cache_dir: PathBuf,
}

fn parse_config(ctx: &Context) -> Config {
    let tikz_cfg = ctx.config.get("preprocessor").and_then(|p| p.get("tikz"));

    let get = |key: &str| -> Option<String> { tikz_cfg?.get(key)?.as_str().map(String::from) };

    let tex_command = get("tex_command")
        .or_else(detect_tex_engine)
        .unwrap_or_else(|| "pdflatex".into());
    let pdf2svg_command = get("pdf2svg_command").unwrap_or_else(|| "pdf2svg".into());
    let preamble = get("preamble").unwrap_or_default();
    let cache_dir = ctx.root.join(".mdbook-tikz-cache");

    Config {
        preamble,
        tex_command,
        pdf2svg_command,
        cache_dir,
    }
}

// ── Block detection ──

enum BlockKind {
    Tikz,
    Tikzcd,
}

struct Block {
    kind: BlockKind,
    full_match: String,
    content: String,
}

fn find_blocks(text: &str) -> Vec<Block> {
    let tikzcd_re =
        Regex::new(r"(?ms)\$\$\s*\\begin\{tikzcd\}(.*?)\\end\{tikzcd\}\s*\$\$").unwrap();
    let tikzpic_re =
        Regex::new(r"(?ms)\$\$\s*\\begin\{tikzpicture\}(.*?)\\end\{tikzpicture\}\s*\$\$")
            .unwrap();

    let from_tikzcd = tikzcd_re.captures_iter(text).map(|cap| Block {
        kind: BlockKind::Tikzcd,
        full_match: cap[0].to_string(),
        content: cap[1].to_string(),
    });

    let from_tikzpic = tikzpic_re.captures_iter(text).map(|cap| Block {
        kind: BlockKind::Tikz,
        full_match: cap[0].to_string(),
        content: format!(
            "\\begin{{tikzpicture}}{}\\end{{tikzpicture}}",
            &cap[1]
        ),
    });

    from_tikzcd.chain(from_tikzpic).collect()
}

// ── Process a chapter ──

fn process_content(content: &str, cfg: &Config) -> String {
    let blocks = find_blocks(content);
    if blocks.is_empty() {
        return content.to_string();
    }

    let mut result = content.to_string();
    for block in &blocks {
        let kind = match block.kind {
            BlockKind::Tikz => "tikz",
            BlockKind::Tikzcd => "tikzcd",
        };
        let latex = wrap_tikz_latex(&block.content, kind, &cfg.preamble);
        let replacement =
            match compile_tikz(&latex, &cfg.cache_dir, &cfg.tex_command, &cfg.pdf2svg_command) {
                Ok(svg) => wrap_svg_html(&svg),
                Err(e) => format!(
                    "<pre class=\"tikz-error\" style=\"color:red;white-space:pre-wrap\">{}</pre>",
                    html_escape(&e)
                ),
            };
        result = result.replacen(&block.full_match, &replacement, 1);
    }
    result
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── Main ──

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // supports subcommand
    if args.len() >= 3 && args[1] == "supports" {
        process::exit(if args[2] == "html" { 0 } else { 1 });
    }

    // Read [context, book] from stdin
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).expect("read stdin");

    let parsed: (Context, Book) = serde_json::from_str(&input).expect("parse JSON input");
    let (ctx, mut book) = parsed;
    let cfg = parse_config(&ctx);

    for section in &mut book.items {
        if let BookItem::Chapter { Chapter: ch } = section {
            process_chapter(ch, &cfg);
        }
    }

    let stdout = io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, &book).expect("write JSON output");
    lock.flush().expect("flush stdout");
}

fn process_chapter(ch: &mut Chapter, cfg: &Config) {
    ch.content = process_content(&ch.content, cfg);
    if let Some(subs) = ch.extra.get_mut("sub_items") {
        if let Some(arr) = subs.as_array_mut() {
            for item in arr {
                if let Ok(mut sub) = serde_json::from_value::<BookItem>(item.clone()) {
                    if let BookItem::Chapter {
                        Chapter: ref mut sub_ch,
                    } = sub
                    {
                        process_chapter(sub_ch, cfg);
                        *item = serde_json::to_value(&sub).unwrap();
                    }
                }
            }
        }
    }
}
