use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::Command;

pub const TEX_FONT_SIZE: &str = "12pt";
pub const SVG_SCALE_FACTOR: f64 = 1.5;
pub const TIKZ_STYLE: &str = "display:block;text-align:center;margin:1em 0";

/// Wrap rendered SVG in a self-styled HTML container.
pub fn wrap_svg_html(svg: &str) -> String {
    format!("<div style=\"{TIKZ_STYLE}\">{svg}</div>")
}

/// Returns the first available TeX engine, or None.
pub fn detect_tex_engine() -> Option<String> {
    for cmd in ["tectonic", "pdflatex"] {
        if Command::new("which")
            .arg(cmd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(cmd.into());
        }
    }
    None
}

/// Wrap TikZ/tikzcd source into a standalone LaTeX document.
pub fn wrap_tikz_latex(source: &str, kind: &str, preamble: &str) -> String {
    let body = if kind == "tikzcd" {
        format!("\\begin{{tikzcd}}\n{}\n\\end{{tikzcd}}", source.trim())
    } else {
        source.to_string()
    };

    let extra = if preamble.is_empty() {
        String::new()
    } else {
        format!("\n{preamble}")
    };

    format!(
        "\\documentclass[crop,tikz,{TEX_FONT_SIZE}]{{standalone}}\n\
         \\usepackage{{tikz-cd}}{extra}\n\
         \\usepackage[T1]{{fontenc}}\n\
         \\usepackage{{lmodern}}\n\
         \\DeclareMathAlphabet{{\\mathtt}}{{T1}}{{lmtt}}{{b}}{{n}}\n\
         \\begin{{document}}\n\
         {body}\n\
         \\end{{document}}\n"
    )
}

/// Compile LaTeX to SVG. Returns the post-processed SVG string.
pub fn compile_tikz(
    latex: &str,
    cache_dir: &Path,
    tex_command: &str,
    pdf2svg_command: &str,
) -> Result<String, String> {
    let hash = format!("{:x}", Sha256::digest(latex.as_bytes()));
    let svg_cache = cache_dir.join(format!("{hash}.svg"));

    if svg_cache.exists() {
        return fs::read_to_string(&svg_cache).map_err(|e| format!("cache read: {e}"));
    }

    fs::create_dir_all(cache_dir).map_err(|e| format!("mkdir cache: {e}"))?;
    let work_dir = cache_dir.join(&hash);
    fs::create_dir_all(&work_dir).map_err(|e| format!("mkdir work: {e}"))?;

    let tex_path = work_dir.join("input.tex");
    let pdf_path = work_dir.join("input.pdf");
    fs::write(&tex_path, latex).map_err(|e| format!("write tex: {e}"))?;

    let svg = run_tex(tex_command, &tex_path, &pdf_path, &work_dir)
        .and_then(|_| run_pdf2svg(pdf2svg_command, &pdf_path, &work_dir))
        .map(|svg| postprocess_svg(&svg));

    let _ = fs::remove_dir_all(&work_dir);

    if let Ok(ref svg) = svg {
        let _ = fs::write(&svg_cache, svg);
    }

    svg
}

fn run_tex(engine: &str, tex_path: &Path, pdf_path: &Path, work_dir: &Path) -> Result<(), String> {
    let output = if engine == "tectonic" {
        Command::new(engine)
            .args(["--outdir", &work_dir.to_string_lossy()])
            .arg(tex_path)
            .output()
    } else {
        Command::new(engine)
            .args(["-interaction=nonstopmode", "-halt-on-error"])
            .arg(format!("-output-directory={}", work_dir.display()))
            .arg(tex_path)
            .output()
    }
    .map_err(|e| format!("{engine}: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "{engine} failed:\n{}",
            String::from_utf8_lossy(&output.stdout)
        ));
    }
    if !pdf_path.exists() {
        return Err("TeX produced no PDF".into());
    }
    Ok(())
}

fn run_pdf2svg(cmd: &str, pdf_path: &Path, work_dir: &Path) -> Result<String, String> {
    let svg_path = work_dir.join("output.svg");
    let output = Command::new(cmd)
        .arg(pdf_path)
        .arg(&svg_path)
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "{cmd} failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    fs::read_to_string(&svg_path).map_err(|e| format!("read svg: {e}"))
}

fn postprocess_svg(svg: &str) -> String {
    let svg = strip_xml_declaration(svg);
    let svg = svg
        .replace("fill=\"rgb(0%, 0%, 0%)\"", "fill=\"currentColor\"")
        .replace("stroke=\"rgb(0%, 0%, 0%)\"", "stroke=\"currentColor\"")
        .replace("fill=\"#000000\"", "fill=\"currentColor\"")
        .replace("stroke=\"#000000\"", "stroke=\"currentColor\"");
    scale_svg_dimensions(&svg, SVG_SCALE_FACTOR)
}

fn strip_xml_declaration(svg: &str) -> &str {
    if let Some(pos) = svg.find("<svg") {
        &svg[pos..]
    } else {
        svg
    }
}

fn scale_svg_dimensions(svg: &str, factor: f64) -> String {
    let re = regex::Regex::new(r#"(width|height)="([0-9.]+)pt""#).unwrap();
    re.replace_all(svg, |caps: &regex::Captures| {
        let attr = &caps[1];
        let val: f64 = caps[2].parse().unwrap_or(0.0);
        format!("{}=\"{:.2}pt\"", attr, val * factor)
    })
    .into_owned()
}
