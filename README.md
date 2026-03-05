# mdbook-tikz

An [mdBook](https://rust-lang.github.io/mdBook/) preprocessor that renders TikZ and tikzcd diagrams to inline SVG.

## Install

```sh
cargo install mdbook-tikz
```

Requires a TeX engine and `pdf2svg` on PATH:

```sh
brew install tectonic pdf2svg   # macOS
```

## Usage

Add to your `book.toml`:

```toml
[preprocessor.tikz]
command = "mdbook-tikz"
```

Then write diagrams in your markdown using display-math syntax:

```markdown
$$
\begin{tikzcd}
A \arrow[r, "f"] \arrow[d, "g"'] & B \arrow[d, "h"] \\
C \arrow[r, "k"'] & D
\end{tikzcd}
$$

$$
\begin{tikzpicture}
\draw (0,0) -- (1,1) -- (2,0) -- cycle;
\end{tikzpicture}
$$
```

The preprocessor detects `$$\begin{tikzcd}...\end{tikzcd}$$` and `$$\begin{tikzpicture}...\end{tikzpicture}$$` blocks, compiles them to PDF via tectonic/pdflatex, converts to SVG via pdf2svg, and inlines the SVG directly in the HTML output.

## Configuration

```toml
[preprocessor.tikz]
command = "mdbook-tikz"
preamble = "\\usepackage{amsmath}"   # extra LaTeX preamble
tex_command = "tectonic"              # override TeX engine (default: auto-detect)
pdf2svg_command = "pdf2svg"           # override converter (default: pdf2svg)
```

## Features

- **SHA-256 caching** — compiled SVGs are cached in `.mdbook-tikz-cache/`. Only recompiles when source changes.
- **Theme-aware** — black fills/strokes are replaced with `currentColor` so diagrams follow your mdBook theme.
- **Display-math sizing** — SVG dimensions are scaled to match KaTeX display math.
- **Library crate** — use `mdbook_tikz` as a dependency for programmatic TikZ rendering.

## Library usage

```rust
use mdbook_tikz::{detect_tex_engine, wrap_tikz_latex, compile_tikz};

let engine = detect_tex_engine().expect("no TeX engine found");
let latex = wrap_tikz_latex(r"\arrow[r] & B", "tikzcd", "");
let svg = compile_tikz(&latex, cache_dir, &engine, "pdf2svg")?;
```

## License

MIT
