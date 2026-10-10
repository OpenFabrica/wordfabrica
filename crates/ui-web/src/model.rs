//! The ribbon as data: tabs → groups → controls, each control naming the command it runs.
//! It mirrors the egui ribbon (`wordfabrica_ui_egui::ribbon`) so both front ends offer the same
//! commands; the HTML view only decides how it looks. Plain data keeps it testable natively.

use serde_json::{Value, json};
use wordfabrica_ui_egui::ChromeState;

/// A command and its parameters (JSON text, parsed when clicked).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cmd {
    pub id: &'static str,
    pub params: &'static str,
    /// Split buttons whose main half reuses the colour picked last (`{"color": …}`).
    pub last: Option<Slot>,
}

/// Which remembered colour a split button uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    Highlight,
    FontColor,
    Shading,
}

impl Slot {
    /// Key in [`ChromeState::last_colors`].
    pub fn key(self) -> &'static str {
        match self {
            Slot::Highlight => "highlight",
            Slot::FontColor => "fontColor",
            Slot::Shading => "shading",
        }
    }
}

/// A command without parameters.
pub const fn c(id: &'static str) -> Cmd {
    Cmd { id, params: "{}", last: None }
}

/// A command with JSON parameters.
pub const fn cp(id: &'static str, params: &'static str) -> Cmd {
    Cmd { id, params, last: None }
}

/// A colour command whose parameter is the remembered colour.
pub const fn cl(id: &'static str, slot: Slot) -> Cmd {
    Cmd { id, params: "{}", last: Some(slot) }
}

impl Cmd {
    /// The parameters to send, given the current state (remembered colours filled in).
    pub fn params(&self, state: &ChromeState) -> Value {
        let mut v: Value = serde_json::from_str(self.params).unwrap_or_else(|_| json!({}));
        if let Some(slot) = self.last
            && let Some(obj) = v.as_object_mut()
        {
            let color = state.last_colors.get(slot.key()).cloned().unwrap_or_default();
            obj.insert("color".into(), json!(color));
        }
        v
    }
}

/// A menu entry.
#[derive(Clone, Copy, Debug)]
pub enum Item {
    /// A command (the shortcut shows beside it).
    Cmd(&'static str, Cmd),
    /// A command shown with a check mark while toggle `check` is on.
    Check(&'static str, Cmd, &'static str),
    Sep,
    Header(&'static str),
    Sub(&'static str, &'static [Item]),
    /// A colour grid (theme colours and tints, standard colours) running `id` with `{"color": hex}`;
    /// `slot` remembers the pick for the split button.
    Colors {
        id: &'static str,
        slot: Option<Slot>,
    },
    /// The highlighter palette.
    Highlights,
    /// A grid of characters, each running `id` with `{key: char}`.
    Glyphs {
        id: &'static str,
        key: &'static str,
        chars: &'static [&'static str],
    },
    /// Insert ▸ Table: hover to size, click to insert.
    TableGrid,
    /// Document language (whole document).
    Language,
    /// Lists that come from the engine.
    Themes,
    ThemeColors,
    ThemeFonts,
    PaperSizes,
    ListLevels,
}

/// A ribbon control.
#[derive(Clone, Copy, Debug)]
pub enum Ctl {
    /// 32 px icon over a (possibly two-line) label; `check` names a toggle drawn pressed.
    Big { icon: &'static str, label: &'static str, cmd: Cmd, check: Option<&'static str> },
    /// 16 px icon, optional label.
    Small { icon: &'static str, label: Option<&'static str>, tip: &'static str, cmd: Cmd, check: Option<&'static str> },
    /// Main half runs `cmd`; the arrow opens `menu`.
    Split { icon: &'static str, tip: &'static str, cmd: Cmd, check: Option<&'static str>, swatch: Option<Slot>, menu: &'static [Item] },
    /// A button that only opens a menu.
    Menu { icon: &'static str, label: Option<&'static str>, tip: &'static str, big: bool, menu: &'static [Item] },
    /// Small controls stacked in rows.
    Rows(&'static [&'static [Ctl]]),
    /// Controls drawn by a dedicated view.
    Special(Special),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Special {
    FontCombo,
    SizeCombo,
    StyleGallery,
    StyleSetGallery,
    TableStyles,
    TableLook,
    /// Layout ▸ Paragraph: indents and spacing fields.
    ParaFields,
    /// View ▸ Show: Ruler, Gridlines, Navigation Pane check boxes.
    ShowChecks,
    /// Table Layout ▸ Alignment: nine cell alignments.
    CellAlign,
    /// Review ▸ Display for Review: "All Markup" / "No Markup".
    MarkupMenu,
}

#[derive(Clone, Copy, Debug)]
pub struct Group {
    /// English title (the dialog launcher's tooltip; groups have no title row, as in WPS).
    pub title: &'static str,
    pub launcher: Option<Cmd>,
    pub ctls: &'static [Ctl],
}

#[derive(Clone, Copy, Debug)]
pub struct Tab {
    pub name: &'static str,
    /// Table Design / Table Layout appear only with the caret in a table.
    pub contextual: bool,
    pub groups: &'static [Group],
}

/// Whether control state `key` is on: `bold`… are character formatting, `align:left`… paragraph
/// alignment, anything else a named toggle in [`ChromeState::toggles`].
pub fn is_checked(state: &ChromeState, key: &str) -> bool {
    let f = &state.format;
    match key {
        "bold" => f.bold,
        "italic" => f.italic,
        "underline" => f.underline,
        "strike" => f.strike,
        "subscript" => f.subscript,
        "superscript" => f.superscript,
        _ => {
            if let Some(a) = key.strip_prefix("align:") {
                state.para.align == a
            } else if let Some(k) = key.strip_prefix('!') {
                !state.toggles.get(k).copied().unwrap_or(false)
            } else {
                state.toggles.get(key).copied().unwrap_or(false)
            }
        }
    }
}

const fn big(icon: &'static str, label: &'static str, cmd: Cmd) -> Ctl {
    Ctl::Big { icon, label, cmd, check: None }
}
const fn big_on(icon: &'static str, label: &'static str, cmd: Cmd, check: &'static str) -> Ctl {
    Ctl::Big { icon, label, cmd, check: Some(check) }
}
const fn small(icon: &'static str, tip: &'static str, cmd: Cmd) -> Ctl {
    Ctl::Small { icon, label: None, tip, cmd, check: None }
}
const fn small_on(icon: &'static str, tip: &'static str, cmd: Cmd, check: &'static str) -> Ctl {
    Ctl::Small { icon, label: None, tip, cmd, check: Some(check) }
}
const fn small_l(icon: &'static str, label: &'static str, cmd: Cmd) -> Ctl {
    Ctl::Small { icon, label: Some(label), tip: label, cmd, check: None }
}
const fn small_lt(icon: &'static str, label: &'static str, tip: &'static str, cmd: Cmd) -> Ctl {
    Ctl::Small { icon, label: Some(label), tip, cmd, check: None }
}
const fn menu_big(icon: &'static str, label: &'static str, tip: &'static str, menu: &'static [Item]) -> Ctl {
    Ctl::Menu { icon, label: Some(label), tip, big: true, menu }
}
const fn menu_small(icon: &'static str, label: Option<&'static str>, tip: &'static str, menu: &'static [Item]) -> Ctl {
    Ctl::Menu { icon, label, tip, big: false, menu }
}
const fn split(icon: &'static str, tip: &'static str, cmd: Cmd, menu: &'static [Item]) -> Ctl {
    Ctl::Split { icon, tip, cmd, check: None, swatch: None, menu }
}
const fn group(title: &'static str, ctls: &'static [Ctl]) -> Group {
    Group { title, launcher: None, ctls }
}
const fn group_l(title: &'static str, launcher: Cmd, ctls: &'static [Ctl]) -> Group {
    Group { title, launcher: Some(launcher), ctls }
}

const fn it(label: &'static str, cmd: Cmd) -> Item {
    Item::Cmd(label, cmd)
}

// ---- Home ----

const PASTE_MENU: &[Item] = &[it("Paste", c("edit.paste")), it("Keep Text Only", c("edit.pasteText")), it("Merge Formatting", c("edit.pasteMerge"))];

const CASE_MENU: &[Item] = &[
    it("Sentence case.", cp("format.changeCase", r#"{"mode":"sentence"}"#)),
    it("lowercase", cp("format.changeCase", r#"{"mode":"lower"}"#)),
    it("UPPERCASE", cp("format.changeCase", r#"{"mode":"upper"}"#)),
    it("Capitalize Each Word", cp("format.changeCase", r#"{"mode":"title"}"#)),
    it("tOGGLE cASE", cp("format.changeCase", r#"{"mode":"toggle"}"#)),
];

const UNDERLINE_MENU: &[Item] = &[
    it("Single", cp("format.underline", r#"{"style":"single"}"#)),
    it("Double", cp("format.underline", r#"{"style":"double"}"#)),
    it("Thick", cp("format.underline", r#"{"style":"thick"}"#)),
    it("Dotted", cp("format.underline", r#"{"style":"dotted"}"#)),
    it("Dashed", cp("format.underline", r#"{"style":"dash"}"#)),
    it("Dot-dash", cp("format.underline", r#"{"style":"dotDash"}"#)),
    it("Wave", cp("format.underline", r#"{"style":"wave"}"#)),
    it("Words only", cp("format.underline", r#"{"style":"words"}"#)),
];

const EFFECTS_MENU: &[Item] = &[
    it("Outline", c("format.outline")),
    it("Shadow", c("format.shadow")),
    it("Small Caps", c("format.smallCaps")),
    it("All Caps", c("format.allCaps")),
    it("Double Strikethrough", c("format.doubleStrikethrough")),
];

const HIGHLIGHT_MENU: &[Item] = &[Item::Highlights, it("No Color", cp("format.highlight", r#"{"color":"none"}"#))];

const FONT_COLOR_MENU: &[Item] =
    &[it("Automatic", cp("format.color", r#"{"color":"auto"}"#)), Item::Colors { id: "format.color", slot: Some(Slot::FontColor) }];

const BULLETS_MENU: &[Item] = &[
    Item::Header("Bullet Library"),
    Item::Glyphs { id: "para.bullets", key: "kind", chars: &["•", "○", "▪", "◆", "➢", "✓", "–"] },
    it("None", cp("para.bullets", r#"{"off":true}"#)),
];

const NUMBERING_MENU: &[Item] = &[
    it("1. 2. 3.", cp("para.numbering", r#"{"kind":"numbered"}"#)),
    it("1) 2) 3)", cp("para.numbering", r#"{"kind":"numberedParen"}"#)),
    it("I. II. III.", cp("para.numbering", r#"{"kind":"outline"}"#)),
    it("A. B. C.", cp("para.numbering", r#"{"kind":"upperLetter"}"#)),
    it("a) b) c)", cp("para.numbering", r#"{"kind":"lowerLetter"}"#)),
    it("i. ii. iii.", cp("para.numbering", r#"{"kind":"lowerRoman"}"#)),
    Item::Sep,
    it("Restart at 1", c("para.restartNumbering")),
    it("None", cp("para.numbering", r#"{"off":true}"#)),
];

const MULTILEVEL_MENU: &[Item] = &[
    it("1. 1.1. 1.1.1.", cp("para.multilevel", r#"{"kind":"legal"}"#)),
    it("I. A. 1. a.", cp("para.multilevel", r#"{"kind":"outline"}"#)),
    Item::Sep,
    Item::ListLevels,
];

const LINE_SPACING_MENU: &[Item] = &[
    it("1", cp("para.lineSpacing", r#"{"value":1.0}"#)),
    it("1.15", cp("para.lineSpacing", r#"{"value":1.15}"#)),
    it("1.5", cp("para.lineSpacing", r#"{"value":1.5}"#)),
    it("2", cp("para.lineSpacing", r#"{"value":2.0}"#)),
    it("2.5", cp("para.lineSpacing", r#"{"value":2.5}"#)),
    it("3", cp("para.lineSpacing", r#"{"value":3.0}"#)),
    Item::Sep,
    it("Add Space Before Paragraph", c("para.addSpaceBefore")),
    it("Remove Space After Paragraph", c("para.removeSpaceAfter")),
    it("Line Spacing Options…", c("para.dialog")),
];

const SHADING_MENU: &[Item] =
    &[it("No Color", cp("para.shading", r#"{"color":null}"#)), Item::Colors { id: "para.shading", slot: Some(Slot::Shading) }];

const PARA_BORDERS_MENU: &[Item] = &[
    it("Bottom Border", cp("para.borders", r#"{"kind":"bottom"}"#)),
    it("Top Border", cp("para.borders", r#"{"kind":"top"}"#)),
    it("Left Border", cp("para.borders", r#"{"kind":"left"}"#)),
    it("Right Border", cp("para.borders", r#"{"kind":"right"}"#)),
    it("No Border", cp("para.borders", r#"{"kind":"none"}"#)),
    it("All Borders", cp("para.borders", r#"{"kind":"all"}"#)),
    it("Outside Borders", cp("para.borders", r#"{"kind":"outside"}"#)),
    it("Inside Borders", cp("para.borders", r#"{"kind":"inside"}"#)),
    Item::Sep,
    it("Horizontal Line", c("insert.horizontalLine")),
];

const SELECT_MENU: &[Item] =
    &[it("Select All", c("select.all")), it("Select Paragraph", c("select.paragraph")), it("Select Sentence", c("select.sentence"))];

const HOME: &[Group] = &[
    group(
        "Clipboard",
        &[
            big_on("painter", "Format\nPainter", c("edit.formatPainter"), "painter"),
            menu_big("paste", "Paste", "Paste", PASTE_MENU),
            Ctl::Rows(&[&[small("cut", "Cut", c("edit.cut"))], &[small("copy", "Copy", c("edit.copy"))]]),
        ],
    ),
    group_l(
        "Font",
        c("format.fontDialog"),
        &[Ctl::Rows(&[
            &[
                Ctl::Special(Special::FontCombo),
                Ctl::Special(Special::SizeCombo),
                small("grow", "Increase Font Size", c("format.growFont")),
                small("shrink", "Decrease Font Size", c("format.shrinkFont")),
                menu_small("case", None, "Change Case", CASE_MENU),
                small("clear", "Clear All Formatting", c("format.clear")),
            ],
            &[
                small_on("bold", "Bold", c("format.bold"), "bold"),
                small_on("italic", "Italic", c("format.italic"), "italic"),
                Ctl::Split {
                    icon: "underline",
                    tip: "Underline",
                    cmd: c("format.underline"),
                    check: Some("underline"),
                    swatch: None,
                    menu: UNDERLINE_MENU,
                },
                small_on("strike", "Strikethrough", c("format.strikethrough"), "strike"),
                small_on("subscript", "Subscript", c("format.subscript"), "subscript"),
                small_on("superscript", "Superscript", c("format.superscript"), "superscript"),
                menu_small("effects", None, "Text Effects and Typography", EFFECTS_MENU),
                split("highlight", "Text Highlight Color", cl("format.highlight", Slot::Highlight), HIGHLIGHT_MENU),
                Ctl::Split {
                    icon: "fontcolor",
                    tip: "Font Color",
                    cmd: cl("format.color", Slot::FontColor),
                    check: None,
                    swatch: Some(Slot::FontColor),
                    menu: FONT_COLOR_MENU,
                },
            ],
        ])],
    ),
    group_l(
        "Paragraph",
        c("para.dialog"),
        &[Ctl::Rows(&[
            &[
                split("bullets", "Bullets", c("para.bullets"), BULLETS_MENU),
                split("numbering", "Numbering", c("para.numbering"), NUMBERING_MENU),
                split("multilevel", "Multilevel List", c("para.multilevel"), MULTILEVEL_MENU),
                small("outdent", "Decrease Indent", c("para.outdent")),
                small("indent", "Increase Indent", c("para.indent")),
                small("sort", "Sort", c("para.sort")),
                small_on("pilcrow", "Show/Hide ¶", c("view.marks"), "marks"),
            ],
            &[
                small_on("alignLeft", "Align Left", c("para.alignLeft"), "align:left"),
                small_on("alignCenter", "Center", c("para.alignCenter"), "align:center"),
                small_on("alignRight", "Align Right", c("para.alignRight"), "align:right"),
                small_on("justify", "Justify", c("para.justify"), "align:justify"),
                menu_small("lineSpacing", None, "Line and Paragraph Spacing", LINE_SPACING_MENU),
                split("shading", "Shading", cl("para.shading", Slot::Shading), SHADING_MENU),
                split("borders", "Borders", cp("para.borders", r#"{"kind":"bottom"}"#), PARA_BORDERS_MENU),
            ],
        ])],
    ),
    group_l("Styles", c("styles.pane"), &[Ctl::Special(Special::StyleGallery)]),
    group(
        "Editing",
        &[
            big("find", "Find", cp("ui.dialog", r#"{"name":"find"}"#)),
            Ctl::Rows(&[
                &[small_lt("replace", "Replace", "Replace", cp("ui.dialog", r#"{"name":"replace"}"#))],
                &[Ctl::Menu { icon: "select", label: Some("Select"), tip: "Select", big: false, menu: SELECT_MENU }],
            ]),
        ],
    ),
    group("Voice", &[big("dictate", "Dictate", c("tools.dictate"))]),
    group("Editor", &[big("editor", "Editor", c("review.spelling"))]),
];

// ---- Insert ----

const COVER_MENU: &[Item] = &[it("Studio cover", c("insert.coverPage"))];
const TABLE_MENU: &[Item] = &[
    Item::TableGrid,
    Item::Sep,
    it("Insert Table…", cp("ui.dialog", r#"{"name":"insertTable"}"#)),
    it("Convert Text to Table…", c("table.fromText")),
    Item::Sub(
        "Quick Tables",
        &[
            it("Tabular List", cp("table.quick", r#"{"kind":"tabular"}"#)),
            it("Matrix", cp("table.quick", r#"{"kind":"matrix"}"#)),
            it("Calendar", cp("table.quick", r#"{"kind":"calendar"}"#)),
        ],
    ),
];
const SHAPES_MENU: &[Item] = &[
    it("Rectangle", cp("insert.shape", r#"{"kind":"rectangle"}"#)),
    it("Rounded Rectangle", cp("insert.shape", r#"{"kind":"roundedRectangle"}"#)),
    it("Oval", cp("insert.shape", r#"{"kind":"ellipse"}"#)),
    it("Triangle", cp("insert.shape", r#"{"kind":"triangle"}"#)),
    it("Diamond", cp("insert.shape", r#"{"kind":"diamond"}"#)),
    it("Line", cp("insert.shape", r#"{"kind":"line"}"#)),
    it("Arrow", cp("insert.shape", r#"{"kind":"arrow"}"#)),
    it("Star", cp("insert.shape", r#"{"kind":"star"}"#)),
    it("Heart", cp("insert.shape", r#"{"kind":"heart"}"#)),
];
const HEADER_MENU: &[Item] = &[
    it("Blank", cp("insert.header", r#"{"preset":"blank"}"#)),
    it("Blank (Three Columns)", cp("insert.header", r#"{"preset":"blankThree"}"#)),
    it("Document Title", cp("insert.header", r#"{"preset":"title"}"#)),
    Item::Sep,
    it("Edit Header", c("insert.editHeader")),
    it("Remove Header", c("insert.removeHeader")),
];
const FOOTER_MENU: &[Item] = &[
    it("Blank", cp("insert.footer", r#"{"preset":"blank"}"#)),
    it("Blank (Three Columns)", cp("insert.footer", r#"{"preset":"blankThree"}"#)),
    it("Page Number", cp("insert.footer", r#"{"preset":"pageNumber"}"#)),
    Item::Sep,
    it("Edit Footer", c("insert.editFooter")),
    it("Remove Footer", c("insert.removeFooter")),
];
const PAGE_NUMBER_MENU: &[Item] = &[
    it("Top of Page", cp("insert.pageNumber", r#"{"position":"top","align":"right"}"#)),
    it("Bottom of Page", cp("insert.pageNumber", r#"{"position":"bottom","align":"center"}"#)),
    it("Page X of Y", cp("insert.pageNumber", r#"{"position":"bottom","format":"x of y"}"#)),
    it("Current Position", cp("insert.pageNumber", r#"{"position":"current"}"#)),
    it("Format Page Numbers…", c("layout.pageNumberFormat")),
];
const SYMBOL_MENU: &[Item] = &[Item::Glyphs {
    id: "insert.symbol",
    key: "char",
    chars: &[
        "©", "®", "™", "§", "¶", "€", "£", "¥", "°", "±", "≠", "≤", "≥", "÷", "×", "∞", "µ", "α", "β", "π", "Ω", "∑", "√", "→", "←", "✓", "★", "♥",
        "—", "…",
    ],
}];

const INSERT: &[Group] = &[
    group(
        "Pages",
        &[Ctl::Rows(&[
            &[menu_small("coverPage", Some("Cover Page"), "Cover Page", COVER_MENU)],
            &[small_l("blankPage", "Blank Page", c("insert.blankPage"))],
            &[small_l("pageBreak", "Page Break", c("insert.pageBreak"))],
        ])],
    ),
    group("Tables", &[menu_big("table", "Table", "Add a Table", TABLE_MENU)]),
    group(
        "Illustrations",
        &[
            big("picture", "Pictures", c("insert.picture")),
            menu_big("shapes", "Shapes", "Shapes", SHAPES_MENU),
            Ctl::Rows(&[
                &[small_l("icons", "Icons", c("insert.icon"))],
                &[small_l("smartArt", "SmartArt", c("insert.smartArt"))],
                &[small_l("chart", "Chart", c("insert.chart"))],
            ]),
        ],
    ),
    group(
        "Links",
        &[Ctl::Rows(&[
            &[small_l("link", "Link", cp("ui.dialog", r#"{"name":"link"}"#))],
            &[small_l("bookmark", "Bookmark", cp("ui.dialog", r#"{"name":"bookmark"}"#))],
            &[small_l("crossRef", "Cross-reference", c("insert.crossReference"))],
        ])],
    ),
    group("Comments", &[big("newComment", "Comment", c("review.newComment"))]),
    group(
        "Header & Footer",
        &[
            menu_big("header", "Header", "Header", HEADER_MENU),
            menu_big("footer", "Footer", "Footer", FOOTER_MENU),
            menu_big("pageNumber", "Page\nNumber", "Page Number", PAGE_NUMBER_MENU),
        ],
    ),
    group(
        "Text",
        &[
            big("textBox", "Text\nBox", c("insert.textBox")),
            Ctl::Rows(&[
                &[small_l("quickParts", "Quick Parts", c("insert.quickParts"))],
                &[small_l("wordArt", "WordArt", c("insert.wordArt"))],
                &[small_l("dropCap", "Drop Cap", c("insert.dropCap"))],
            ]),
            Ctl::Rows(&[
                &[small("signature", "Signature Line", c("insert.signatureLine"))],
                &[small("dateTime", "Date & Time", c("insert.dateTime"))],
                &[small("object", "Object", c("insert.object"))],
            ]),
        ],
    ),
    group("Symbols", &[big("equation", "Equation", c("insert.equation")), menu_big("symbol", "Symbol", "Symbol", SYMBOL_MENU)]),
];

// ---- Draw ----

const DRAW: &[Group] = &[
    group(
        "Drawing Tools",
        &[
            big("select", "Select", c("draw.select")),
            big("lasso", "Lasso", c("draw.lasso")),
            big("eraser", "Eraser", c("draw.eraser")),
            big("pen", "Pen", c("draw.pen")),
            big("pencil", "Pencil", c("draw.pencil")),
            big("highlight", "Highlighter", c("draw.highlighter")),
        ],
    ),
    group("Convert", &[big("inkToShape", "Ink to\nShape", c("draw.inkToShape")), big("inkToMath", "Ink to\nMath", c("draw.inkToMath"))]),
    group("Insert", &[big("canvas", "Drawing\nCanvas", c("insert.canvas"))]),
    group("Replay", &[big("replay", "Ink\nReplay", c("draw.replay"))]),
];

// ---- Design ----

const PARA_SPACING_MENU: &[Item] = &[
    it("Default", cp("design.paragraphSpacing", r#"{"value":"default"}"#)),
    it("No Paragraph Space", cp("design.paragraphSpacing", r#"{"value":"none"}"#)),
    it("Compact", cp("design.paragraphSpacing", r#"{"value":"compact"}"#)),
    it("Tight", cp("design.paragraphSpacing", r#"{"value":"tight"}"#)),
    it("Open", cp("design.paragraphSpacing", r#"{"value":"open"}"#)),
    it("Relaxed", cp("design.paragraphSpacing", r#"{"value":"relaxed"}"#)),
    it("Double", cp("design.paragraphSpacing", r#"{"value":"double"}"#)),
];
const WATERMARK_MENU: &[Item] = &[
    it("DRAFT", cp("design.watermark", r#"{"text":"DRAFT"}"#)),
    it("CONFIDENTIAL", cp("design.watermark", r#"{"text":"CONFIDENTIAL"}"#)),
    it("DO NOT COPY", cp("design.watermark", r#"{"text":"DO NOT COPY"}"#)),
    it("SAMPLE", cp("design.watermark", r#"{"text":"SAMPLE"}"#)),
    it("ASAP", cp("design.watermark", r#"{"text":"ASAP"}"#)),
    it("URGENT", cp("design.watermark", r#"{"text":"URGENT"}"#)),
    Item::Sep,
    it("Custom Watermark…", cp("ui.dialog", r#"{"name":"watermark"}"#)),
    it("Remove Watermark", cp("design.watermark", r#"{"remove":true}"#)),
];
const PAGE_COLOR_MENU: &[Item] = &[it("No Color", cp("design.pageColor", r#"{"color":null}"#)), Item::Colors { id: "design.pageColor", slot: None }];
const PAGE_BORDERS_MENU: &[Item] =
    &[it("Box", cp("design.pageBorders", r#"{"kind":"box"}"#)), it("None", cp("design.pageBorders", r#"{"kind":"none"}"#))];

const DESIGN: &[Group] = &[
    group("Themes", &[menu_big("themes", "Themes", "Themes", &[Item::Themes])]),
    group(
        "Document Formatting",
        &[
            Ctl::Special(Special::StyleSetGallery),
            Ctl::Rows(&[
                &[menu_small("colors", Some("Colors"), "Theme Colors", &[Item::ThemeColors])],
                &[menu_small("fonts", Some("Fonts"), "Theme Fonts", &[Item::ThemeFonts])],
            ]),
            Ctl::Rows(&[
                &[menu_small("paraSpacing", Some("Paragraph Spacing"), "Paragraph Spacing", PARA_SPACING_MENU)],
                &[small_l("effectsDesign", "Effects", c("design.effects"))],
                &[small_l("setDefault", "Set as Default", c("design.setDefault"))],
            ]),
        ],
    ),
    group(
        "Page Background",
        &[
            menu_big("watermark", "Watermark", "Watermark", WATERMARK_MENU),
            menu_big("pageColor", "Page\nColor", "Page Color", PAGE_COLOR_MENU),
            menu_big("pageBorders", "Page\nBorders", "Page Borders", PAGE_BORDERS_MENU),
        ],
    ),
];

// ---- Layout ----

const MARGINS_MENU: &[Item] = &[
    it("Normal  1\" all", cp("layout.margins", r#"{"preset":"normal"}"#)),
    it("Narrow  0.5\" all", cp("layout.margins", r#"{"preset":"narrow"}"#)),
    it("Moderate", cp("layout.margins", r#"{"preset":"moderate"}"#)),
    it("Wide", cp("layout.margins", r#"{"preset":"wide"}"#)),
    it("Mirrored", cp("layout.margins", r#"{"preset":"mirrored"}"#)),
    it("Office 2003 Default", cp("layout.margins", r#"{"preset":"office2003"}"#)),
    it("Custom Margins…", cp("ui.dialog", r#"{"name":"pageSetup"}"#)),
];
const ORIENTATION_MENU: &[Item] =
    &[it("Portrait", cp("layout.orientation", r#"{"value":"portrait"}"#)), it("Landscape", cp("layout.orientation", r#"{"value":"landscape"}"#))];
const COLUMNS_MENU: &[Item] = &[
    it("One", cp("layout.columns", r#"{"count":1}"#)),
    it("Two", cp("layout.columns", r#"{"count":2}"#)),
    it("Three", cp("layout.columns", r#"{"count":3}"#)),
    it("Left", cp("layout.columns", r#"{"preset":"left"}"#)),
    it("Right", cp("layout.columns", r#"{"preset":"right"}"#)),
];
const BREAKS_MENU: &[Item] = &[
    Item::Header("Page Breaks"),
    it("Page", cp("layout.break", r#"{"kind":"page"}"#)),
    it("Column", cp("layout.break", r#"{"kind":"column"}"#)),
    it("Text Wrapping", cp("layout.break", r#"{"kind":"textWrapping"}"#)),
    Item::Header("Section Breaks"),
    it("Next Page", cp("layout.break", r#"{"kind":"nextPage"}"#)),
    it("Continuous", cp("layout.break", r#"{"kind":"continuous"}"#)),
    it("Even Page", cp("layout.break", r#"{"kind":"evenPage"}"#)),
    it("Odd Page", cp("layout.break", r#"{"kind":"oddPage"}"#)),
];
const LINE_NUMBERS_MENU: &[Item] = &[
    it("None", cp("layout.lineNumbers", r#"{"value":"none"}"#)),
    it("Continuous", cp("layout.lineNumbers", r#"{"value":"continuous"}"#)),
    it("Restart Each Page", cp("layout.lineNumbers", r#"{"value":"restartPage"}"#)),
    it("Restart Each Section", cp("layout.lineNumbers", r#"{"value":"restartSection"}"#)),
];
const HYPHENATION_MENU: &[Item] = &[
    Item::Check("Automatic", cp("layout.hyphenation", r#"{"value":true}"#), "hyphenation"),
    Item::Check("None", cp("layout.hyphenation", r#"{"value":false}"#), "!hyphenation"),
];

const LAYOUT: &[Group] = &[
    group_l(
        "Page Setup",
        cp("ui.dialog", r#"{"name":"pageSetup"}"#),
        &[
            menu_big("margins", "Margins", "Margins", MARGINS_MENU),
            menu_big("orientation", "Orientation", "Orientation", ORIENTATION_MENU),
            menu_big("size", "Size", "Size", &[Item::PaperSizes]),
            menu_big("columns", "Columns", "Columns", COLUMNS_MENU),
            Ctl::Rows(&[
                &[menu_small("breaks", Some("Breaks"), "Breaks", BREAKS_MENU)],
                &[menu_small("lineNumbers", Some("Line Numbers"), "Line Numbers", LINE_NUMBERS_MENU)],
                &[menu_small("hyphenation", Some("Hyphenation"), "Hyphenation", HYPHENATION_MENU)],
            ]),
        ],
    ),
    group_l("Paragraph", c("para.dialog"), &[Ctl::Special(Special::ParaFields)]),
    group(
        "Arrange",
        &[
            big("position", "Position", c("arrange.position")),
            big("wrapText", "Wrap\nText", c("arrange.wrap")),
            Ctl::Rows(&[
                &[small_l("bringForward", "Bring Forward", c("arrange.bringForward"))],
                &[small_l("sendBackward", "Send Backward", c("arrange.sendBackward"))],
                &[small_l("selectionPane", "Selection Pane", c("arrange.selectionPane"))],
            ]),
            Ctl::Rows(&[
                &[small("align", "Align", c("arrange.align"))],
                &[small("group", "Group", c("arrange.group"))],
                &[small("rotate", "Rotate", c("arrange.rotate"))],
            ]),
        ],
    ),
];

// ---- References ----

const TOC_MENU: &[Item] = &[
    it("Automatic Table 1 (3 levels)", cp("references.toc", r#"{"levels":3}"#)),
    it("Automatic Table 2 (2 levels)", cp("references.toc", r#"{"levels":2,"title":"Table of Contents"}"#)),
    it("Remove Table of Contents", c("references.removeToc")),
];
const ADD_TEXT_MENU: &[Item] = &[
    it("Do Not Show in TOC", cp("references.addText", r#"{"level":0}"#)),
    it("Level 1", cp("references.addText", r#"{"level":1}"#)),
    it("Level 2", cp("references.addText", r#"{"level":2}"#)),
    it("Level 3", cp("references.addText", r#"{"level":3}"#)),
];

const REFERENCES: &[Group] = &[
    group(
        "Table of Contents",
        &[
            menu_big("toc", "Table of\nContents", "Table of Contents", TOC_MENU),
            Ctl::Rows(&[
                &[menu_small("addText", Some("Add Text"), "Add Text", ADD_TEXT_MENU)],
                &[small_l("updateTable", "Update Table", c("references.updateToc"))],
            ]),
        ],
    ),
    group(
        "Footnotes",
        &[
            big("footnote", "Insert\nFootnote", c("references.footnote")),
            Ctl::Rows(&[
                &[small_l("endnote", "Insert Endnote", c("references.endnote"))],
                &[small_l("nextFootnote", "Next Footnote", c("references.nextFootnote"))],
                &[small_l("showNotes", "Show Notes", c("references.notes"))],
            ]),
        ],
    ),
    group("Research", &[big("researcher", "Researcher", c("references.researcher"))]),
    group(
        "Citations & Bibliography",
        &[
            big("citation", "Insert\nCitation", c("references.citation")),
            Ctl::Rows(&[
                &[small_l("sources", "Manage Sources", c("references.sources"))],
                &[small_lt("styles", "Style: APA", "Citation Style", c("references.citationStyle"))],
                &[small_l("bibliography", "Bibliography", c("references.bibliography"))],
            ]),
        ],
    ),
    group(
        "Captions",
        &[
            big("caption", "Insert\nCaption", c("references.caption")),
            Ctl::Rows(&[
                &[small_l("tableOfFigures", "Insert Table of Figures", c("references.tableOfFigures"))],
                &[small_lt("update", "Update Table", "Update Table of Figures", c("references.updateFigures"))],
                &[small_l("crossRef", "Cross-reference", c("insert.crossReference"))],
            ]),
        ],
    ),
    group(
        "Index",
        &[
            big("markEntry", "Mark\nEntry", c("references.markEntry")),
            Ctl::Rows(&[
                &[small_l("index", "Insert Index", c("references.index"))],
                &[small_l("update", "Update Index", c("references.updateIndex"))],
            ]),
        ],
    ),
    group(
        "Table of Authorities",
        &[
            big("markCitation", "Mark\nCitation", c("references.markCitation")),
            small("tableOfAuthorities", "Insert Table of Authorities", c("references.tableOfAuthorities")),
        ],
    ),
];

// ---- Review ----

/// Review ▸ Tracking ▸ Display for Review (its label follows the current setting).
pub const MARKUP_MENU: &[Item] =
    &[it("All Markup", cp("review.markup", r#"{"value":"all"}"#)), it("No Markup", cp("review.markup", r#"{"value":"noMarkup"}"#))];
const ACCEPT_MENU: &[Item] = &[it("Accept This Change", c("review.accept")), it("Accept All Changes", c("review.acceptAll"))];
const REJECT_MENU: &[Item] = &[it("Reject Change", c("review.reject")), it("Reject All Changes", c("review.rejectAll"))];

const REVIEW: &[Group] = &[
    group(
        "Proofing",
        &[
            big("spelling", "Spelling &\nGrammar", c("review.spelling")),
            Ctl::Rows(&[
                &[small_l("thesaurus", "Thesaurus", c("review.thesaurus"))],
                &[small_l("wordCount", "Word Count", cp("ui.dialog", r#"{"name":"wordCount"}"#))],
            ]),
        ],
    ),
    group("Speech", &[big("readAloud", "Read\nAloud", c("review.readAloud"))]),
    group("Accessibility", &[big("accessibility", "Check\nAccessibility", c("file.accessibility"))]),
    group(
        "Language",
        &[big("translate", "Translate", c("review.translate")), menu_big("language", "Language", "Document language", &[Item::Language])],
    ),
    group(
        "Comments",
        &[
            big("newComment", "New\nComment", c("review.newComment")),
            Ctl::Rows(&[
                &[small_lt("deleteComment", "Delete", "Delete Comment", c("review.deleteComment"))],
                &[small_lt("prevComment", "Previous", "Previous Comment", c("review.previousComment"))],
                &[small_lt("nextComment", "Next", "Next Comment", c("review.nextComment"))],
            ]),
            // Shows or hides the balloons beside the page; the list lives in the Comments pane.
            big_on("showComments", "Show\nComments", c("view.showComments"), "showComments"),
        ],
    ),
    group(
        "Tracking",
        &[
            big_on("trackChanges", "Track\nChanges", c("review.trackChanges"), "trackChanges"),
            Ctl::Rows(&[&[Ctl::Special(Special::MarkupMenu)], &[small_l("reviewingPane", "Reviewing Pane", c("review.changes"))]]),
        ],
    ),
    group(
        "Changes",
        &[
            menu_big("accept", "Accept", "Accept", ACCEPT_MENU),
            menu_big("reject", "Reject", "Reject", REJECT_MENU),
            Ctl::Rows(&[
                &[small_lt("prevChange", "Previous", "Previous Change", c("review.previousChange"))],
                &[small_lt("nextChange", "Next", "Next Change", c("review.nextChange"))],
            ]),
        ],
    ),
    group("Compare", &[big("compare", "Compare", c("review.compare"))]),
    group("Protect", &[big("blockAuthors", "Block\nAuthors", c("review.blockAuthors")), big("restrict", "Restrict\nEditing", c("review.restrict"))]),
];

// ---- View ----

const VIEW: &[Group] = &[
    group(
        "Views",
        &[
            big("readMode", "Read\nMode", c("view.readMode")),
            big("printLayout", "Print\nLayout", c("view.printLayout")),
            big("webLayout", "Web\nLayout", c("view.webLayout")),
            Ctl::Rows(&[&[small_l("outline", "Outline", c("view.outline"))], &[small_l("draft", "Draft", c("view.draft"))]]),
        ],
    ),
    group("Immersive", &[big("focus", "Focus", c("view.focus")), big("immersive", "Immersive\nReader", c("view.immersive"))]),
    group("Page Movement", &[big("vertical", "Vertical", c("view.vertical")), big("sideToSide", "Side\nto Side", c("view.sideToSide"))]),
    group("Show", &[Ctl::Special(Special::ShowChecks)]),
    group(
        "Zoom",
        &[
            big("zoom", "Zoom", cp("ui.dialog", r#"{"name":"zoom"}"#)),
            big("zoom100", "100%", c("view.zoom100")),
            Ctl::Rows(&[
                &[Ctl::Small { icon: "onePage", label: Some("One Page"), tip: "One Page", cmd: c("view.onePage"), check: Some("onePage") }],
                &[Ctl::Small {
                    icon: "multiplePages",
                    label: Some("Multiple Pages"),
                    tip: "Multiple Pages",
                    cmd: c("view.multiplePages"),
                    check: Some("multiplePages"),
                }],
                &[Ctl::Small { icon: "pageWidth", label: Some("Page Width"), tip: "Page Width", cmd: c("view.pageWidth"), check: Some("pageWidth") }],
            ]),
        ],
    ),
    group("Dark Mode", &[big("darkMode", "Switch\nModes", c("view.darkMode"))]),
    group(
        "Window",
        &[
            big("newWindow", "New\nWindow", c("view.newWindow")),
            big("arrangeAll", "Arrange\nAll", c("view.arrangeAll")),
            big("split", "Split", c("view.split")),
        ],
    ),
    group("Macros", &[big("macros", "Macros", c("tools.macros"))]),
];

// ---- Help ----

const HELP: &[Group] = &[
    group("Help", &[big("help", "Help", cp("ui.dialog", r#"{"name":"about"}"#)), big("community", "Community\nDiscussions", c("ui.community"))]),
    group("Agents", &[big("macros", "Commands", cp("ui.dialog", r#"{"name":"commands"}"#))]),
];

// ---- Table tools (contextual) ----

const TABLE_SHADING_MENU: &[Item] =
    &[it("No Color", cp("table.shading", r#"{"color":null}"#)), Item::Colors { id: "table.shading", slot: Some(Slot::Shading) }];
const TABLE_BORDERS_MENU: &[Item] = &[
    it("All Borders", cp("table.borders", r#"{"kind":"all"}"#)),
    it("Outside Borders", cp("table.borders", r#"{"kind":"outside"}"#)),
    it("Inside Borders", cp("table.borders", r#"{"kind":"inside"}"#)),
    it("No Border", cp("table.borders", r#"{"kind":"none"}"#)),
    it("Top Border", cp("table.borders", r#"{"kind":"top"}"#)),
    it("Bottom Border", cp("table.borders", r#"{"kind":"bottom"}"#)),
];

const TABLE_DESIGN: &[Group] = &[
    group("Table Style Options", &[Ctl::Special(Special::TableLook)]),
    group(
        "Table Styles",
        &[Ctl::Special(Special::TableStyles), Ctl::Rows(&[&[split("shading", "Shading", cl("table.shading", Slot::Shading), TABLE_SHADING_MENU)]])],
    ),
    group(
        "Borders",
        &[menu_big("borders", "Borders", "Borders", TABLE_BORDERS_MENU), big("borderPainter", "Border\nPainter", c("table.borderPainter"))],
    ),
];

const TABLE_SELECT_MENU: &[Item] =
    &[it("Select Cell", c("table.selectCell")), it("Select Row", c("table.selectRow")), it("Select Table", c("table.selectTable"))];
const TABLE_DELETE_MENU: &[Item] = &[
    it("Delete Cells", c("table.deleteCells")),
    it("Delete Columns", c("table.deleteColumn")),
    it("Delete Rows", c("table.deleteRow")),
    it("Delete Table", c("table.deleteTable")),
];
const AUTOFIT_MENU: &[Item] = &[
    it("AutoFit Contents", cp("table.autofit", r#"{"mode":"contents"}"#)),
    it("AutoFit Window", cp("table.autofit", r#"{"mode":"window"}"#)),
    it("Fixed Column Width", cp("table.autofit", r#"{"mode":"fixed"}"#)),
];

const TABLE_LAYOUT: &[Group] = &[
    group(
        "Table",
        &[Ctl::Rows(&[
            &[menu_small("selectTable", Some("Select"), "Select", TABLE_SELECT_MENU)],
            &[Ctl::Small {
                icon: "gridlines",
                label: Some("View Gridlines"),
                tip: "View Gridlines",
                cmd: c("view.gridlines"),
                check: Some("gridlines"),
            }],
            &[small_lt("properties", "Properties", "Table Properties", c("table.properties"))],
        ])],
    ),
    group(
        "Rows & Columns",
        &[
            menu_big("deleteTable", "Delete", "Delete", TABLE_DELETE_MENU),
            big("insertAbove", "Insert\nAbove", c("table.insertRowAbove")),
            Ctl::Rows(&[
                &[small_l("insertBelow", "Insert Below", c("table.insertRowBelow"))],
                &[small_l("insertLeft", "Insert Left", c("table.insertColumnLeft"))],
                &[small_l("insertRight", "Insert Right", c("table.insertColumnRight"))],
            ]),
        ],
    ),
    group(
        "Merge",
        &[Ctl::Rows(&[
            &[small_l("merge", "Merge Cells", c("table.merge"))],
            &[small_l("splitCells", "Split Cells", c("table.split"))],
            &[small_l("splitTable", "Split Table", c("table.splitTable"))],
        ])],
    ),
    group(
        "Cell Size",
        &[
            menu_big("autofit", "AutoFit", "AutoFit", AUTOFIT_MENU),
            Ctl::Rows(&[
                &[small_l("distributeRows", "Distribute Rows", c("table.distributeRows"))],
                &[small_l("distributeCols", "Distribute Columns", c("table.distributeColumns"))],
            ]),
        ],
    ),
    group(
        "Alignment",
        &[
            Ctl::Special(Special::CellAlign),
            big("textDirection", "Text\nDirection", c("table.textDirection")),
            big("cellMargins", "Cell\nMargins", c("table.cellMargins")),
        ],
    ),
    group(
        "Data",
        &[
            big("sort", "Sort", c("table.sort")),
            Ctl::Rows(&[
                &[small_l("repeatHeader", "Repeat Header Rows", c("table.repeatHeader"))],
                &[small_l("toText", "Convert to Text", c("table.toText"))],
                &[small_l("formula", "Formula", c("table.formula"))],
            ]),
        ],
    ),
];

/// Every tab, in strip order. File opens from the title bar.
pub const TABS: &[Tab] = &[
    Tab { name: "Home", contextual: false, groups: HOME },
    Tab { name: "Insert", contextual: false, groups: INSERT },
    Tab { name: "Draw", contextual: false, groups: DRAW },
    Tab { name: "Design", contextual: false, groups: DESIGN },
    Tab { name: "Layout", contextual: false, groups: LAYOUT },
    Tab { name: "References", contextual: false, groups: REFERENCES },
    Tab { name: "Review", contextual: false, groups: REVIEW },
    Tab { name: "View", contextual: false, groups: VIEW },
    Tab { name: "Help", contextual: false, groups: HELP },
    Tab { name: "Table Design", contextual: true, groups: TABLE_DESIGN },
    Tab { name: "Table Layout", contextual: true, groups: TABLE_LAYOUT },
];

/// The tab named `name` (Home when unknown, as the egui ribbon does).
pub fn tab(name: &str) -> &'static Tab {
    TABS.iter().find(|t| t.name == name).unwrap_or(&TABS[0])
}

/// Nine cell alignments, row by row (Table Layout ▸ Alignment).
pub const CELL_ALIGN: [[&str; 3]; 3] =
    [["topLeft", "topCenter", "topRight"], ["centerLeft", "center", "centerRight"], ["bottomLeft", "bottomCenter", "bottomRight"]];

/// Table Style Options: (label, `table.look` key, state field).
pub const TABLE_LOOK: [(&str, &str); 6] = [
    ("Header Row", "headerRow"),
    ("First Column", "firstColumn"),
    ("Total Row", "totalRow"),
    ("Last Column", "lastColumn"),
    ("Banded Rows", "bandedRows"),
    ("Banded Columns", "bandedColumns"),
];

/// Word's standard colours (bottom row of every colour grid).
pub const STANDARD_COLORS: [&str; 10] = ["C00000", "FF0000", "FFC000", "FFFF00", "92D050", "00B050", "00B0F0", "0070C0", "002060", "7030A0"];

/// The theme part of a colour grid: six rows (base colours, then lighter and darker tints) of the
/// ten theme colours in Word's column order, as hex. Same arithmetic as the egui grid.
pub fn theme_color_rows(theme: &[String]) -> Vec<Vec<String>> {
    let base: Vec<wordfabrica_doc::Rgb> = theme.iter().take(10).filter_map(|h| wordfabrica_doc::Rgb::parse(h)).collect();
    let tint = |c: wordfabrica_doc::Rgb, k: f32| {
        let f = |v: u8| if k >= 0.0 { (v as f32 + (255.0 - v as f32) * k) as u8 } else { (v as f32 * (1.0 + k)) as u8 };
        wordfabrica_doc::Rgb(f(c.0), f(c.1), f(c.2))
    };
    let order = [1usize, 0, 3, 2, 4, 5, 6, 7, 8, 9];
    [0.0f32, 0.8, 0.6, 0.4, -0.25, -0.5].iter().map(|&k| order.iter().filter_map(|&i| base.get(i).map(|c| tint(*c, k).hex())).collect()).collect()
}

/// The design style sets offered in Design ▸ Document Formatting.
pub const STYLE_SETS: [&str; 7] = ["default", "basic", "lines", "shaded", "casual", "centered", "minimalist"];

/// Commands in the title bar, tab strip and status bar (beside the ribbon).
pub const CHROME_IDS: &[&str] = &[
    "file.save",
    "edit.undo",
    "edit.redo",
    "ui.dialog",
    "ui.tab",
    "ui.backstage",
    "ui.collapseRibbon",
    "ui.community",
    "review.trackChanges",
    "view.commentsPane",
    "view.zoom",
    "view.zoomIn",
    "view.zoomOut",
    "view.webLayout",
    "view.printLayout",
    "view.readMode",
    "view.focus",
    "review.language",
    "view.ruler",
    "view.gridlines",
    "view.navigationPane",
    "table.look",
    "table.style",
    "table.cellAlign",
    "para.style",
    "para.indents",
    "para.spacing",
    "format.font",
    "format.size",
    "design.styleSet",
    "design.theme",
    "design.themeColors",
    "design.themeFonts",
    "layout.size",
    "para.listLevel",
    "insert.table",
    "review.markup",
    "format.highlight",
];

/// The commands of a menu and its sub-menus.
fn menu_ids(items: &[Item], out: &mut Vec<&'static str>) {
    for i in items {
        match i {
            Item::Cmd(_, cmd) | Item::Check(_, cmd, _) => out.push(cmd.id),
            Item::Sub(_, sub) => menu_ids(sub, out),
            Item::Colors { id, .. } | Item::Glyphs { id, .. } => out.push(id),
            Item::Highlights => out.push("format.highlight"),
            Item::Sep
            | Item::Header(_)
            | Item::TableGrid
            | Item::Language
            | Item::Themes
            | Item::ThemeColors
            | Item::ThemeFonts
            | Item::PaperSizes
            | Item::ListLevels => {}
        }
    }
}

fn ctl_ids(ctl: &Ctl, out: &mut Vec<&'static str>) {
    match ctl {
        Ctl::Big { cmd, .. } | Ctl::Small { cmd, .. } => out.push(cmd.id),
        Ctl::Split { cmd, menu, .. } => {
            out.push(cmd.id);
            menu_ids(menu, out);
        }
        Ctl::Menu { menu, .. } => menu_ids(menu, out),
        Ctl::Rows(rows) => rows.iter().flat_map(|r| r.iter()).for_each(|c| ctl_ids(c, out)),
        Ctl::Special(_) => {}
    }
}

/// Every command id the web chrome can run: the snapshot reports which are disabled.
pub fn all_ids() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = CHROME_IDS.to_vec();
    for t in TABS {
        for g in t.groups {
            if let Some(l) = &g.launcher {
                out.push(l.id);
            }
            for ctl in g.ctls {
                ctl_ids(ctl, &mut out);
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every command the HTML ribbon offers exists (UI commands live in the UI layer).
    #[test]
    fn every_ribbon_command_exists() {
        // Word features not built yet: the egui ribbon shows them too, greyed out (the snapshot
        // reports unknown ids as disabled). Anything else unknown is a typo.
        const NOT_YET: &[&str] = &[
            "arrange.group",
            "draw.eraser",
            "draw.highlighter",
            "draw.inkToMath",
            "draw.inkToShape",
            "draw.lasso",
            "draw.pen",
            "draw.pencil",
            "draw.replay",
            "draw.select",
            "insert.canvas",
            "insert.chart",
            "insert.icon",
            "insert.smartArt",
            "review.blockAuthors",
            "review.translate",
            "tools.dictate",
            "view.arrangeAll",
        ];
        let reg = wordfabrica_engine::cmd::registry();
        let missing: Vec<_> = all_ids().into_iter().filter(|id| !id.starts_with("ui.") && reg.get(id).is_none() && !NOT_YET.contains(id)).collect();
        assert!(missing.is_empty(), "unknown commands {missing:?}");
    }

    fn all_cmds(items: &[Item], out: &mut Vec<Cmd>) {
        for i in items {
            match i {
                Item::Cmd(_, cmd) | Item::Check(_, cmd, _) => out.push(*cmd),
                Item::Sub(_, sub) => all_cmds(sub, out),
                _ => {}
            }
        }
    }

    /// Parameters are JSON objects (a typo would silently become `{}` at click time).
    #[test]
    fn every_parameter_is_a_json_object() {
        let mut cmds = Vec::new();
        for t in TABS {
            for g in t.groups {
                cmds.extend(g.launcher);
                let mut stack: Vec<&Ctl> = g.ctls.iter().collect();
                while let Some(ctl) = stack.pop() {
                    match ctl {
                        Ctl::Big { cmd, .. } | Ctl::Small { cmd, .. } => cmds.push(*cmd),
                        Ctl::Split { cmd, menu, .. } => {
                            cmds.push(*cmd);
                            all_cmds(menu, &mut cmds);
                        }
                        Ctl::Menu { menu, .. } => all_cmds(menu, &mut cmds),
                        Ctl::Rows(rows) => stack.extend(rows.iter().flat_map(|r| r.iter())),
                        Ctl::Special(_) => {}
                    }
                }
            }
        }
        assert!(cmds.len() > 150, "only {} commands", cmds.len());
        for cmd in cmds {
            let v: Value = serde_json::from_str(cmd.params).unwrap_or_else(|e| panic!("{}: {e}", cmd.id));
            assert!(v.is_object(), "{}: {}", cmd.id, cmd.params);
        }
    }

    /// Every icon the ribbon names is drawn (not the lettered fallback tile).
    #[test]
    fn every_ribbon_icon_is_drawn() {
        let fallback = |name: &str| wordfabrica_ui_egui::icons::svg(name, 20.0, Default::default(), Default::default()).contains("<rect x=\"2.400\"");
        let mut icons = Vec::new();
        for t in TABS {
            for g in t.groups {
                let mut stack: Vec<&Ctl> = g.ctls.iter().collect();
                while let Some(ctl) = stack.pop() {
                    match ctl {
                        Ctl::Big { icon, .. } | Ctl::Small { icon, .. } | Ctl::Split { icon, .. } | Ctl::Menu { icon, .. } => icons.push(*icon),
                        Ctl::Rows(rows) => stack.extend(rows.iter().flat_map(|r| r.iter())),
                        Ctl::Special(_) => {}
                    }
                }
            }
        }
        let missing: Vec<_> = icons.into_iter().filter(|i| fallback(i)).collect();
        // The egui ribbon draws these with the same fallback; keep the list from growing.
        assert!(missing.len() <= 40, "icons without artwork: {missing:?}");
    }

    #[test]
    fn remembered_colours_fill_split_parameters() {
        let mut st = ChromeState::default();
        st.last_colors.insert("highlight".into(), "green".into());
        assert_eq!(cl("format.highlight", Slot::Highlight).params(&st), json!({"color": "green"}));
        assert_eq!(cp("view.zoom", r#"{"value":100}"#).params(&st), json!({"value": 100}));
    }

    #[test]
    fn checked_states_read_format_alignment_and_toggles() {
        let mut st = ChromeState::default();
        st.format.bold = true;
        st.para.align = "center".into();
        st.toggles.insert("marks".into(), true);
        assert!(is_checked(&st, "bold") && !is_checked(&st, "italic"));
        assert!(is_checked(&st, "align:center") && !is_checked(&st, "align:left"));
        assert!(is_checked(&st, "marks") && !is_checked(&st, "unknown"));
    }

    #[test]
    fn colour_grid_has_tints_and_survives_bad_input() {
        let theme: Vec<String> = ["000000", "FFFFFF", "44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5", "70AD47"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let rows = theme_color_rows(&theme);
        assert_eq!(rows.len(), 6);
        assert!(rows.iter().all(|r| r.len() == 10));
        assert_eq!(rows[0][0], "FFFFFF");
        assert!(theme_color_rows(&["zz".into()]).iter().all(|r| r.is_empty()));
    }

    #[test]
    fn unknown_tab_falls_back_to_home() {
        assert_eq!(tab("Nope").name, "Home");
        assert_eq!(tab("Table Layout").name, "Table Layout");
    }
}
