<p align="center">
  <a href="https://openfabrica.dev/">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="docs/brand/openfabrica-logo-white.svg">
      <img alt="OpenFabrica" src="docs/brand/openfabrica-logo.svg" width="200">
    </picture>
  </a>
</p>

<h1 align="center">WordFabrica</h1>

<p align="center">
  <b>Writing and document design; an open-source, clean-room reimplementation of Microsoft Word, rebuilt in pure Rust.</b>
</p>

<p align="center">
  A fast, open-source word processor with the Word workflow you already know: the ribbon, styles,
  tables, track changes, references and mail merge. It reads and writes .docx, runs natively on
  macOS, Windows, Linux and BSD, and in the browser via WebAssembly.
</p>

<p align="center">
  <img alt="Written in Rust" src="https://img.shields.io/badge/written%20in-Rust-2b47b5?style=flat-square&logo=rust&logoColor=white">
  <img alt="Runs on macOS, Windows, Linux, BSD and the web" src="https://img.shields.io/badge/runs%20on-macOS%20%C2%B7%20Windows%20%C2%B7%20Linux%20%C2%B7%20BSD%20%C2%B7%20Web-3b5bdb?style=flat-square">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-2b47b5?style=flat-square">
  <img alt="Agent-drivable over MCP" src="https://img.shields.io/badge/agents-MCP%20%C2%B7%20CLI-3b5bdb?style=flat-square">
</p>

<p align="center">
  Part of <a href="https://github.com/OpenFabrica">OpenFabrica</a> — open source tools and models for makers, builders, and creatives.
</p>

<p align="center">
  <a href="https://github.com/OpenFabrica/wordfabrica"><b>WordFabrica on GitHub</b></a> ·
  <a href="https://openfabrica.dev/">openfabrica.dev</a> ·
  <a href="https://github.com/OpenFabrica/wordfabrica/releases/latest">Download</a>
</p>

<br>

<p align="center">
  <img src="docs/images/hero.png" alt="WordFabrica with the Home tab of the ribbon open over a two-page document titled The Open Studio Handbook. The Navigation pane on the left lists the document's headings, the Styles gallery shows live previews of Normal, Heading 1, Title and Subtitle, and a word in the first paragraph is selected." width="100%">
  <br><sub><b>The Open Studio Handbook</b>, WordFabrica's built-in sample: the ribbon, live Styles gallery, rulers and the Navigation pane.</sub>
</p>

<p align="center">
  <a href="#a-tour">A tour</a> ·
  <a href="#why-wordfabrica">Why WordFabrica</a> ·
  <a href="#what-works-today">What works today</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#for-agents-cli-and-mcp">For agents</a> ·
  <a href="#architecture">Architecture</a> ·
  <a href="#roadmap">Roadmap</a> ·
  <a href="#downloads">Downloads</a> ·
  <a href="#license-and-credits">License and credits</a>
</p>

## A tour

Every screenshot below is WordFabrica itself, rendered offscreen by its own UI test harness
(`cargo run -p wordfabrica-ui-egui --example ui_shot`).

<table>
<tr>
<td width="50%" valign="top"><img src="docs/images/review.png" alt="The Review tab with Track Changes on: the word forty is inserted in magenta and thirty struck through; three commented phrases are shaded and joined by dashed leader lines to comment balloons in a grey markup area to the right of the page" width="100%"><p align="center"><sub><b>Review.</b> Track changes, comment balloons in the margin, accept and reject, spelling and grammar as you type.</sub></p></td>
<td width="50%" valign="top"><img src="docs/images/references.png" alt="The References tab with two pages side by side: a table of contents with dotted leaders and page numbers on page one, and a styled table, numbered list and hyperlink on page two" width="100%"><p align="center"><sub><b>References.</b> Tables of contents, footnotes, citations in APA, MLA, Chicago or IEEE, index and captions.</sub></p></td>
</tr>
<tr>
<td width="50%" valign="top"><img src="docs/images/design.png" alt="The Design tab showing style-set previews; the document is set in a serif theme with plum headings underlined by thin rules and a pale diagonal DRAFT watermark behind the text" width="100%"><p align="center"><sub><b>Design.</b> Themes, style sets, paragraph spacing, watermarks, page colour and borders.</sub></p></td>
<td width="50%" valign="top"><img src="docs/images/dark.png" alt="WordFabrica in dark mode with the Insert tab open and formatting marks shown: pilcrows at paragraph ends and dots for spaces" width="100%"><p align="center"><sub><b>Dark mode</b> with formatting marks, and the Insert tab: tables, pictures, shapes, links, headers, footers, fields and symbols.</sub></p></td>
</tr>
<tr>
<td colspan="2"><img src="docs/images/layout.png" alt="The Layout tab at 80% zoom: the first paragraph opens with a three-line drop cap E, a dark blue circle sits beside the second section with its paragraph wrapping around it and hyphenating art-ist at the line end, every line is numbered in the left margin and a thin blue border surrounds the page" width="100%"><p align="center"><sub><b>Layout.</b> Drop caps, text wrapping around pictures and shapes, automatic hyphenation, line numbers and page borders.</sub></p></td>
</tr>
<tr>
<td colspan="2"><img src="docs/images/backstage.png" alt="The File tab start page with a Good morning greeting, thumbnails of the Blank document, Studio handbook, Letter, Résumé and Report templates, and a list of recent documents" width="100%"><p align="center"><sub><b>File.</b> Start from a template, open recent documents, edit properties, export to PDF and other formats.</sub></p></td>
</tr>
</table>

## Why WordFabrica

- **Familiar.** Word's ribbon tabs, groups, shortcuts and behaviour: Enter continues a list,
  Tab demotes it, Ctrl/⌘+B bolds the word under the caret, the Styles gallery previews styles live,
  F4 repeats, F7 checks spelling, F8 extends the selection.
- **Your files.** Opens and saves .docx (OOXML), and also .odt, .rtf, .html, .md, .txt; exports PDF
  with real, selectable text, links and bookmarks.
- **Fast.** Paragraph layout is cached, so typing in a 188-page document re-lays it out in about
  1.4 ms; pages render on demand.
- **Everywhere.** One Rust codebase for macOS, Windows, Linux, BSD and the web. No Electron, no
  Tauri: native [egui](https://github.com/emilk/egui) on the GPU.
- **Built for agents.** Every action is a command with an id. The same 389 commands drive the
  ribbon, keyboard shortcuts, the command search, a command-line tool, a JSON control channel and
  an MCP server.
- **Private.** Spelling, grammar and everything else work offline.
- **Open.** MIT OR Apache-2.0. Clean-room: built from public specifications and observation, with
  every asset original or openly licensed.

## What works today

| Area | Highlights |
|---|---|
| **Writing** | Fast typing with IME, smart quotes, AutoCorrect, list autoformat (`* `, `1. `), dashes; word, sentence, paragraph selection; drag-select; clipboard with formatting; undo/redo; find and replace with regex |
| **Formatting** | Fonts, sizes, bold/italic/underline styles, strike, sub/superscript, caps, highlight, colours, character spacing, Format Painter, Change Case, Clear Formatting |
| **Paragraphs** | Alignment, indents (draggable on the ruler), spacing, line spacing, tabs with leaders, borders, shading, keep with next, widow/orphan control |
| **Styles** | Built-in style set, live gallery, Styles pane, create/modify/update styles, style sets, themes |
| **Lists** | Bullets, numbering, multilevel, restart, set value, custom formats |
| **Tables** | Insert by grid, merge/split, styles with banded rows, borders, shading, header rows repeated across pages, rows that split across pages, sort, formulas, text ↔ table |
| **Pages** | Margins, orientation, size, columns, page/column/section breaks, headers and footers (first page, odd/even), page numbers, watermark, page borders, line numbers, vertical alignment, drop caps, automatic hyphenation |
| **Objects** | Pictures (resize, crop, recolour, brightness/contrast, transparency, background removal, picture styles, rotate), shapes, text boxes, floating position with text wrapping (square, top and bottom, behind or in front of text) |
| **References** | Table of contents, footnotes and endnotes, citations and bibliography (APA, MLA, Chicago, IEEE), captions, table of figures, cross-references, index, table of authorities |
| **Review** | Spelling and grammar with suggestions, thesaurus, word count, comments in margin balloons or a pane, track changes, accept/reject, compare documents, restrict editing, accessibility checker, document inspector |
| **Mailings** | Mail merge from CSV, merge fields, address block, greeting line, rules, preview, finish to a document; envelopes and labels |
| **View** | Print layout, web layout, draft, read mode, focus, zoom, one/multiple pages, page width, Navigation pane, rulers, gridlines, dark mode; interface in English, 简体中文, 繁體中文 or 日本語 (follows the system language by default) |
| **Files** | .docx read/write (opens in Word), PDF export, .odt, .rtf, .html, .md, .txt import/export, page images |

The honest picture, area by area, is in [ROADMAP.md](ROADMAP.md) and the generated
[feature parity report](docs/parity.md).

## Quick start

```sh
git clone https://github.com/OpenFabrica/wordfabrica
cd wordfabrica
cargo run --release -p wordfabrica -- --sample        # the desktop app with the sample document
cargo run --release -p wordfabrica -- report.docx     # open a document
```

Command line:

```sh
wordfabrica-cli convert report.docx report.pdf        # docx, pdf, odt, rtf, html, md, txt, png
wordfabrica-cli text report.docx                      # plain text
wordfabrica-cli inspect report.docx                   # structure as JSON
wordfabrica-cli run --template sample \
  --cmd 'select.text={"text":"Membership"}' --cmd format.bold --save out.docx
```

Web with server storage and Chinese/English switching: prepare the external CJK font input as described
in [the setup guide](docs/web-storage.md), build with `cd apps/wordfabrica-web && trunk build --release`, then from
 the repository root run `cargo run -p wordfabrica-server`. Open <http://127.0.0.1:8771/>.
 The single Rust process serves the WASM editor and saves versioned documents in embedded SQLite.
Fonts live in `resources/fonts/` in this repository, versioned with `fonts-v*` tags. No separate font project is required. Server fonts default to `resources/fonts/server/` and can be extended through `WORDFABRICA_FONTS_DIR` without rebuilding WASM; see [font setup](docs/web-fonts.md).
 See [server architecture and setup](docs/web-storage.md) and [browser editing help](docs/web-help.md).

Standalone browser demo: `cd apps/wordfabrica-web && trunk serve`, then open
<http://127.0.0.1:8771/?local&sample> (Save downloads to your computer).

## For agents: CLI and MCP

WordFabrica was designed to be driven by people *and* by AI agents.

```sh
claude mcp add wordfabrica -- wordfabrica-cli mcp                  # headless documents
wordfabrica --control 7981 &                                     # or drive the running app…
claude mcp add wordfabrica-app -- wordfabrica-cli mcp --connect 127.0.0.1:7981
```

Tools include `list_commands`, `execute`, `batch`, `type_text`, `select_text`, `inspect_document`,
`render_page`, `save_document`, and, with a running app, `screenshot`, `click` and `key`. Agents
can check their work through `inspect_document` without screenshots. See [docs/mcp.md](docs/mcp.md)
and the [control protocol](docs/control-protocol.md). Macros record any sequence of commands and
play it back (`tools.recordMacro`, `tools.macros`).

## Logs

The desktop app writes its log records to standard error and to `logs/wordfabrica.log` next to its
preferences: on Linux `$XDG_CONFIG_HOME/wordfabrica/logs/` (by default `~/.config/wordfabrica/logs/`),
on macOS `~/Library/Application Support/WordFabrica/logs/`, on Windows `%APPDATA%\WordFabrica\logs\`.
A start from a desktop menu or the Dock has no terminal, so this file is what to attach to a bug
report: unreadable parts of a .docx, pictures or fonts a PDF export had to leave out and panics the
command guard recovered from land there. Each launch moves the previous log to `wordfabrica.1.log`
(and that one to `wordfabrica.2.log`), so the log of a run that crashed survives the next start; a
log stops growing at 16 MiB. `--version` writes no file, and runs with `WORDFABRICA_NO_PREFS` (agents'
test runs) log to standard error only. Document text is never logged; a panic message leaves out
the text it quotes.

By default WordFabrica's own crates log at `info` and everything else at `warn`. `RUST_LOG` replaces
that with env_logger-style directives, for example `RUST_LOG=debug`,
`RUST_LOG=warn,wordfabrica_docx=trace` or `RUST_LOG=info,wgpu_core=warn`; a directive ending in `*`
covers every target starting with it (`wordfabrica*=debug`). The logger is
`apps/wordfabrica/src/logging.rs`.

## Architecture

| Layer | Crate | Job |
|---|---|---|
| L0 | `wordfabrica-geom` | units and measurements |
| L1 | `wordfabrica-doc`, `wordfabrica-fonts`, `wordfabrica-proof` | document model and editing; fonts and shaping; spelling, grammar, hyphenation |
| L2 | `wordfabrica-layout`, `wordfabrica-docx`, `wordfabrica-formats` | line breaking, pagination, tables, notes, hit testing; OOXML; ODT/RTF/HTML/Markdown/TXT |
| L3 | `wordfabrica-render`, `wordfabrica-pdf` | rasteriser (vello_cpu); PDF (krilla) |
| L4 | `wordfabrica-engine` | session, undo, 389 commands, Word feature catalog |
| L5 | `wordfabrica-mcp` | MCP server |
| L6 | `wordfabrica-ui-egui` | the Word-style front end (swappable) |
| apps | `wordfabrica`, `wordfabrica-cli`, `wordfabrica-web` | desktop, command line, browser |

`cargo xtask ci` runs formatting, clippy, ~250 tests, the asset-attribution check, the layering
check and the wasm build. Contributor and agent instructions: [AGENTS.md](AGENTS.md).

## Roadmap

WordFabrica covers 87% of Word's ribbon features with commands today; counting depth and
fidelity, we estimate about 62% of real feature parity. An alpha for everyday writing is close:
the remaining work is mostly testing against real-world .docx files, native printing and the
first signed builds. Charts, SmartArt, the equation editor and the Draw tab come after.
Details and estimates: [ROADMAP.md](ROADMAP.md).

## Downloads

**Download WordFabrica** from GitHub: the [latest release](https://github.com/OpenFabrica/wordfabrica/releases/latest) has every build listed below, and [all releases](https://github.com/OpenFabrica/wordfabrica/releases) has earlier versions and their notes. `<ver>` in the file names is the version number, and `SHA256SUMS.txt` lists a checksum for every file.

### Windows

| Build | Installer | Portable |
|---|---|---|
| x64 (64-bit Intel/AMD) | `wordfabrica-<ver>-windows-x64.msi` | `wordfabrica-<ver>-windows-x64-portable.zip` |
| arm64 (Snapdragon and other ARM PCs) | `wordfabrica-<ver>-windows-arm64.msi` | `wordfabrica-<ver>-windows-arm64-portable.zip` |
| x86 (32-bit) | `wordfabrica-<ver>-windows-x86.msi` | `wordfabrica-<ver>-windows-x86-portable.zip` |

Installers and executables are code-signed.

**If the app doesn't open on Windows:** the desktop app initializes only DirectX 12 by default.
Letting wgpu also create an OpenGL instance can crash some graphics drivers (AMD's
`atio6axx.dll`) before the window appears, so the app would flash in Task Manager and quit.
`WGPU_BACKEND` overrides the default for troubleshooting (for example `dx12` or `vulkan`). In
PowerShell, from the folder containing the executable:

```powershell
$env:WGPU_BACKEND = "vulkan"
& .\wordfabrica.exe
Remove-Item Env:WGPU_BACKEND                     # restore the default for later launches
```

An explicit `gl` override can bring the driver crash back on affected systems. The macOS, Linux
and web backend defaults are unchanged.

### macOS

| Build | File | Notes |
|---|---|---|
| App, universal (Apple silicon + Intel) | `wordfabrica-<ver>-macos-universal.dmg` | Signed and notarized |
| Command-line tool, universal | `wordfabrica-cli-<ver>-macos-universal.zip` | Signed and notarized |

### Linux

| Format | x86_64 | aarch64 (ARM64) | Notes |
|---|---|---|---|
| AppImage | `wordfabrica-<ver>-linux-x86_64.AppImage` | `wordfabrica-<ver>-linux-aarch64.AppImage` | Runs anywhere; updates itself with [AppImageUpdate](https://github.com/AppImageCommunity/AppImageUpdate) (`.zsync` files) |
| Flatpak | `wordfabrica-<ver>-linux-x86_64.flatpak` | `wordfabrica-<ver>-linux-aarch64.flatpak` | Sandboxed; `flatpak install --user <file>` |
| Debian/Ubuntu | `wordfabrica-<ver>-linux-x86_64.deb` | `wordfabrica-<ver>-linux-aarch64.deb` | |
| Fedora/RHEL/openSUSE | `wordfabrica-<ver>-linux-x86_64.rpm` | `wordfabrica-<ver>-linux-aarch64.rpm` | |
| Tarball | `wordfabrica-<ver>-linux-x86_64.tar.gz` | `wordfabrica-<ver>-linux-aarch64.tar.gz` | Unpack anywhere |

### FreeBSD

| Build | File |
|---|---|
| x86_64 | `wordfabrica-<ver>-freebsd-x86_64.tar.gz` |

### Web (WebAssembly)

| Build | File | Notes |
|---|---|---|
| Static site | `wordfabrica-web-<ver>.zip` | Runs in a modern browser; host it on any static server |

## License and credits

WordFabrica is dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

WordFabrica is a fork of [WordCraft](https://github.com/storytold/wordcraft) by the ArtCraft Team,
used under MIT OR Apache-2.0. Original copyright (c) 2026 ArtCraft Team and the WordCraft
contributors, plus the OpenFabrica contributors for the changes made here. Required notices are in
[NOTICE](NOTICE). OpenFabrica is independent and is not affiliated with ArtCraft. ArtCraft's names
and logos are not used as our brand; the logos in [`docs/brand/`](docs/brand/) are original
OpenFabrica artwork.

Bundled fonts, icons, images and other assets keep their own open licenses; each one is listed
with its author, source and license in [ATTRIBUTION.md](ATTRIBUTION.md).

The spelling dictionary and hyphenation come from Grady Ward's public-domain Moby Hyphenator II
word list. The sample documents and templates are original text written for WordFabrica.

<sub>Microsoft and Microsoft Word are trademarks of the Microsoft group of companies. Adobe, Photoshop, Illustrator, Premiere Pro, Lightroom, Acrobat, After Effects and InDesign are trademarks or registered trademarks of Adobe Inc. in the United States and/or other countries. WordFabrica is an independent, open-source project and is not affiliated with, sponsored by or endorsed by Microsoft Corporation or Adobe Inc.; these names are used only to describe the workflows it is compatible with.</sub>

<p align="center">
  <a href="https://openfabrica.dev/"><img alt="OpenFabrica" src="docs/brand/openfabrica-mark.svg" width="28"></a><br>
  <sub>Part of <a href="https://github.com/OpenFabrica">OpenFabrica</a>.</sub>
</p>
