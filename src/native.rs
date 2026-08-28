//! Native markdown renderer built on Direct2D and DirectWrite.
//!
//! Selected with the `--beta_render` flag. This path draws the document
//! directly instead of handing HTML to WebView2, which removes the browser
//! process, the on-disk WebView2 profile, and the HTML/CSS pipeline from
//! startup.
//!
//! Styling is hardcoded to match the CSS used by the WebView2 path, so there
//! is no cascade, selector matching, or general box model here — just a fixed
//! vertical stack of blocks in a single centred column.
//!
//! Known gaps versus the WebView2 renderer, all deliberate for this beta:
//! text selection and clipboard, clickable links, images (alt text is shown
//! instead), auto-sized table columns, and syntax highlighting.

use pulldown_cmark::{Event as MdEvent, HeadingLevel, Options, Parser, Tag, TagEnd};
use tao::event::{ElementState, Event, MouseButton, MouseScrollDelta, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::keyboard::KeyCode;
use tao::platform::windows::WindowExtWindows;
use tao::window::{Icon, WindowBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct2D::Common::{
    D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_SIZE_U,
};
use windows::Win32::Graphics::Direct2D::{
    D2D1CreateFactory, D2D1_DRAW_TEXT_OPTIONS_NONE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_PRESENT_OPTIONS_NONE,
    D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
    D2D1_ROUNDED_RECT, ID2D1Factory, ID2D1HwndRenderTarget, ID2D1SolidColorBrush,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_ITALIC,
    DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_WEIGHT_SEMI_BOLD,
    DWRITE_LINE_SPACING_METHOD_UNIFORM, DWRITE_TEXT_METRICS, DWRITE_TEXT_RANGE,
    DWRITE_WORD_WRAPPING_WRAP, DWriteCreateFactory, IDWriteFactory, IDWriteFontCollection,
    IDWriteTextFormat, IDWriteTextLayout,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::core::w;
use windows_numerics::Vector2;

// ---------------------------------------------------------------------------
// Theme — mirrors the CSS constants in the WebView2 path.
// ---------------------------------------------------------------------------

const BG: u32 = 0x0d1117;
const FG: u32 = 0xe6edf3;
const FG_STRONG: u32 = 0xf0f6fc;
const FG_MUTED: u32 = 0x8b949e;
const SURFACE: u32 = 0x161b22;
const BORDER: u32 = 0x30363d;
const BORDER_SOFT: u32 = 0x21262d;
const LINK: u32 = 0x58a6ff;

const MAX_COLUMN: f32 = 860.0;
const PAD_X: f32 = 24.0;
const PAD_Y: f32 = 32.0;

const BODY_SIZE: f32 = 16.0;
const BODY_LINE: f32 = 25.6;
const ITEM_LINE: f32 = 27.2;
const CODE_SIZE: f32 = 13.6;
const CODE_LINE: f32 = 19.7;

const BLOCK_GAP: f32 = 16.0;
const HEADING_GAP_ABOVE: f32 = 24.0;
const RULE_GAP: f32 = 24.0;
const CODE_PAD: f32 = 16.0;
const QUOTE_BAR: f32 = 4.0;
const QUOTE_PAD: f32 = 16.0;
const LIST_INDENT: f32 = 32.0;
const ITEM_GAP: f32 = 8.0;
const CELL_PAD_X: f32 = 13.0;
const CELL_PAD_Y: f32 = 6.0;

const THUMB: u32 = 0x30363d;
const THUMB_HOVER: u32 = 0x484f58;
const SCROLLBAR_W: f32 = 10.0;
const THUMB_MIN: f32 = 28.0;

/// Indices into the renderer's brush table.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ink {
    Background,
    Text,
    Strong,
    Muted,
    Surface,
    Border,
    BorderSoft,
    Link,
    Thumb,
    ThumbHover,
}

// ---------------------------------------------------------------------------
// Document model
// ---------------------------------------------------------------------------

/// An inline style applied to a UTF-16 range of a block's text.
#[derive(Clone, Copy)]
struct Span {
    start: u32,
    len: u32,
    style: Style,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Style {
    Strong,
    Emphasis,
    Code,
    Link,
}

/// A run of text with its inline styles, stored as UTF-16 for DirectWrite.
#[derive(Default, Clone)]
struct Text {
    utf16: Vec<u16>,
    spans: Vec<Span>,
}

impl Text {
    fn push(&mut self, s: &str) {
        self.utf16.extend(s.encode_utf16());
    }

    fn cursor(&self) -> u32 {
        self.utf16.len() as u32
    }

    fn style(&mut self, start: u32, style: Style) {
        let len = self.cursor().saturating_sub(start);
        if len > 0 {
            self.spans.push(Span { start, len, style });
        }
    }

    fn is_blank(&self) -> bool {
        self.utf16.iter().all(|c| *c == 0x20 || *c == 0x0a || *c == 0x09)
    }
}

/// Which text style a block is rendered with.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavor {
    Heading(u8),
    Body,
    Code,
    Item,
}

#[derive(Clone)]
struct TableRow {
    cells: Vec<Text>,
    header: bool,
}

#[derive(Clone)]
enum Block {
    /// A run of text. `indent` is added to the left edge; `quote_depth` draws
    /// that many blockquote bars and mutes the text.
    Text {
        flavor: Flavor,
        text: Text,
        indent: f32,
        quote_depth: u32,
        tight: bool,
    },
    Rule,
    Table(Vec<TableRow>),
}

/// Converts markdown source into the flat block list the layout stage consumes.
///
/// Args:
///     source: Raw markdown text.
///
/// Returns:
///     Blocks in document order.
fn parse(source: &str) -> Vec<Block> {
    let parser = Parser::new_ext(source, Options::all());

    let mut blocks: Vec<Block> = Vec::new();
    let mut text = Text::default();
    let mut flavor = Flavor::Body;
    let mut style_starts: Vec<(Style, u32)> = Vec::new();
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut quote_depth: u32 = 0;
    let mut in_block = false;
    let mut tight = false;

    // Table accumulation
    let mut table_rows: Vec<TableRow> = Vec::new();
    let mut row_cells: Vec<Text> = Vec::new();
    let mut in_table = false;
    let mut in_header = false;
    let mut in_cell = false;

    let flush = |text: &mut Text,
                     blocks: &mut Vec<Block>,
                     flavor: Flavor,
                     quote_depth: u32,
                     indent: f32,
                     tight: bool| {
        if !text.utf16.is_empty() && !(text.is_blank() && flavor != Flavor::Code) {
            blocks.push(Block::Text {
                flavor,
                text: std::mem::take(text),
                indent,
                quote_depth,
                tight,
            });
        } else {
            text.utf16.clear();
            text.spans.clear();
        }
    };

    for event in parser {
        match event {
            MdEvent::Start(Tag::Heading { level, .. }) => {
                let n = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                flavor = Flavor::Heading(n);
                in_block = true;
            }
            MdEvent::End(TagEnd::Heading(_)) => {
                let indent = list_indent(&list_stack);
                flush(&mut text, &mut blocks, flavor, quote_depth, indent, false);
                flavor = Flavor::Body;
                in_block = false;
            }

            MdEvent::Start(Tag::Paragraph) => {
                flavor = if list_stack.is_empty() { Flavor::Body } else { Flavor::Item };
                in_block = true;
            }
            MdEvent::End(TagEnd::Paragraph) => {
                let indent = list_indent(&list_stack);
                flush(&mut text, &mut blocks, flavor, quote_depth, indent, tight);
                flavor = Flavor::Body;
                in_block = false;
                tight = false;
            }

            MdEvent::Start(Tag::CodeBlock(_)) => {
                flavor = Flavor::Code;
                in_block = true;
            }
            MdEvent::End(TagEnd::CodeBlock) => {
                // Trailing newline from the fence would render as a blank line.
                while text.utf16.last() == Some(&0x0a) {
                    text.utf16.pop();
                }
                let indent = list_indent(&list_stack);
                flush(&mut text, &mut blocks, Flavor::Code, quote_depth, indent, false);
                flavor = Flavor::Body;
                in_block = false;
            }

            MdEvent::Start(Tag::BlockQuote(_)) => quote_depth += 1,
            MdEvent::End(TagEnd::BlockQuote(_)) => quote_depth = quote_depth.saturating_sub(1),

            MdEvent::Start(Tag::List(start)) => list_stack.push(start),
            MdEvent::End(TagEnd::List(_)) => {
                list_stack.pop();
            }

            MdEvent::Start(Tag::Item) => {
                // The marker is written into the text itself; the block is then
                // indented so wrapped lines align past it.
                let marker = match list_stack.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *n += 1;
                        m
                    }
                    _ => String::from("\u{2022} "),
                };
                text.push(&marker);
                flavor = Flavor::Item;
                in_block = true;
                tight = true;
            }
            MdEvent::End(TagEnd::Item) => {
                let indent = list_indent(&list_stack);
                flush(&mut text, &mut blocks, Flavor::Item, quote_depth, indent, true);
                in_block = false;
            }

            MdEvent::Start(Tag::Table(_)) => {
                in_table = true;
                table_rows.clear();
            }
            MdEvent::End(TagEnd::Table) => {
                if !table_rows.is_empty() {
                    blocks.push(Block::Table(std::mem::take(&mut table_rows)));
                }
                in_table = false;
            }
            MdEvent::Start(Tag::TableHead) => {
                in_header = true;
                row_cells.clear();
            }
            MdEvent::End(TagEnd::TableHead) => {
                table_rows.push(TableRow {
                    cells: std::mem::take(&mut row_cells),
                    header: true,
                });
                in_header = false;
            }
            MdEvent::Start(Tag::TableRow) => row_cells.clear(),
            MdEvent::End(TagEnd::TableRow) => {
                table_rows.push(TableRow {
                    cells: std::mem::take(&mut row_cells),
                    header: false,
                });
            }
            MdEvent::Start(Tag::TableCell) => {
                in_cell = true;
                in_block = true;
            }
            MdEvent::End(TagEnd::TableCell) => {
                row_cells.push(std::mem::take(&mut text));
                in_cell = false;
                in_block = false;
            }

            MdEvent::Start(Tag::Strong) => style_starts.push((Style::Strong, text.cursor())),
            MdEvent::End(TagEnd::Strong) => close_style(&mut text, &mut style_starts, Style::Strong),
            MdEvent::Start(Tag::Emphasis) => style_starts.push((Style::Emphasis, text.cursor())),
            MdEvent::End(TagEnd::Emphasis) => {
                close_style(&mut text, &mut style_starts, Style::Emphasis)
            }
            MdEvent::Start(Tag::Link { .. }) => style_starts.push((Style::Link, text.cursor())),
            MdEvent::End(TagEnd::Link) => close_style(&mut text, &mut style_starts, Style::Link),

            // Images are not decoded in this beta; show the alt text instead.
            MdEvent::Start(Tag::Image { .. }) => {
                style_starts.push((Style::Emphasis, text.cursor()));
                text.push("[image: ");
            }
            MdEvent::End(TagEnd::Image) => {
                text.push("]");
                close_style(&mut text, &mut style_starts, Style::Emphasis);
            }

            MdEvent::Text(t) => {
                if in_block || in_cell {
                    text.push(&t);
                }
            }
            MdEvent::Code(t) => {
                let start = text.cursor();
                text.push(&t);
                text.style(start, Style::Code);
            }
            MdEvent::SoftBreak => text.push(" "),
            MdEvent::HardBreak => text.push("\n"),
            MdEvent::Rule => {
                let indent = list_indent(&list_stack);
                flush(&mut text, &mut blocks, flavor, quote_depth, indent, false);
                blocks.push(Block::Rule);
            }

            // Inline and block HTML is passed through as literal text rather
            // than interpreted; a viewer should not execute markup.
            MdEvent::Html(t) | MdEvent::InlineHtml(t) => {
                if in_block || in_cell {
                    let start = text.cursor();
                    text.push(t.trim_end_matches('\n'));
                    text.style(start, Style::Code);
                }
            }
            _ => {}
        }
    }

    let indent = list_indent(&list_stack);
    flush(&mut text, &mut blocks, flavor, quote_depth, indent, false);
    let _ = in_table;
    let _ = in_header;
    blocks
}

fn list_indent(stack: &[Option<u64>]) -> f32 {
    stack.len() as f32 * LIST_INDENT
}

fn close_style(text: &mut Text, starts: &mut Vec<(Style, u32)>, want: Style) {
    if let Some(pos) = starts.iter().rposition(|(s, _)| *s == want) {
        let (style, start) = starts.remove(pos);
        text.style(start, style);
    }
}

// ---------------------------------------------------------------------------
// Display list
// ---------------------------------------------------------------------------

enum Draw {
    Text { layout: IDWriteTextLayout, x: f32, y: f32, ink: Ink },
    /// `radius` of 0 draws square corners.
    Rect { rect: D2D_RECT_F, ink: Ink, radius: f32 },
    Line { x0: f32, y0: f32, x1: f32, y1: f32, ink: Ink, width: f32 },
}

/// A primitive plus the vertical band it occupies, so painting can cull.
struct Item {
    draw: Draw,
    top: f32,
    bottom: f32,
}

// ---------------------------------------------------------------------------
// Renderer
// ---------------------------------------------------------------------------

struct Renderer {
    dwrite: IDWriteFactory,
    target: ID2D1HwndRenderTarget,
    brushes: Vec<ID2D1SolidColorBrush>,
    body: IDWriteTextFormat,
    item: IDWriteTextFormat,
    code: IDWriteTextFormat,
    headings: Vec<IDWriteTextFormat>,
    blocks: Vec<Block>,
    items: Vec<Item>,
    content_height: f32,
    view: (f32, f32),
    scroll: f32,
    /// Cursor position in DIPs, tracked for scrollbar hover and drag.
    cursor: (f32, f32),
    /// Distance from the top of the thumb to the grab point, while dragging.
    drag_grab: Option<f32>,
    hover_thumb: bool,
}

/// Vertical placement of the scrollbar thumb, in DIPs.
struct Thumb {
    top: f32,
    height: f32,
}

impl Renderer {
    /// Creates the Direct2D target and DirectWrite formats for a window.
    ///
    /// Args:
    ///     hwnd: Target window handle.
    ///     pixels: Window client size in physical pixels.
    ///
    /// Returns:
    ///     A renderer with no document loaded yet.
    fn new(hwnd: isize, pixels: (u32, u32)) -> windows::core::Result<Self> {
        unsafe {
            let factory: ID2D1Factory =
                D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;

            let target_props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                // Zero means "use the system DPI", which makes every coordinate
                // below a DIP and gives correct HiDPI scaling for free.
                dpiX: 0.0,
                dpiY: 0.0,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let hwnd_props = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd: HWND(hwnd as *mut core::ffi::c_void),
                pixelSize: D2D_SIZE_U { width: pixels.0.max(1), height: pixels.1.max(1) },
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };
            let target = factory.CreateHwndRenderTarget(&target_props, &hwnd_props)?;

            let mut brushes = Vec::new();
            for rgb in [
                BG,
                FG,
                FG_STRONG,
                FG_MUTED,
                SURFACE,
                BORDER,
                BORDER_SOFT,
                LINK,
                THUMB,
                THUMB_HOVER,
            ] {
                brushes.push(target.CreateSolidColorBrush(&color(rgb), None)?);
            }

            let dwrite: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;

            let body = make_format(&dwrite, w!("Segoe UI"), BODY_SIZE, BODY_LINE, false)?;
            let item = make_format(&dwrite, w!("Segoe UI"), BODY_SIZE, ITEM_LINE, false)?;
            let code = make_format(&dwrite, w!("Consolas"), CODE_SIZE, CODE_LINE, false)?;

            let mut headings = Vec::new();
            for size in [32.0_f32, 24.0, 20.0, 16.0, 16.0, 16.0] {
                headings.push(make_format(
                    &dwrite,
                    w!("Segoe UI"),
                    size,
                    size * 1.25,
                    true,
                )?);
            }

            Ok(Self {
                dwrite,
                target,
                brushes,
                body,
                item,
                code,
                headings,
                blocks: Vec::new(),
                items: Vec::new(),
                content_height: 0.0,
                view: (0.0, 0.0),
                scroll: 0.0,
                cursor: (-1.0, -1.0),
                drag_grab: None,
                hover_thumb: false,
            })
        }
    }

    fn brush(&self, ink: Ink) -> &ID2D1SolidColorBrush {
        let index = match ink {
            Ink::Background => 0,
            Ink::Text => 1,
            Ink::Strong => 2,
            Ink::Muted => 3,
            Ink::Surface => 4,
            Ink::Border => 5,
            Ink::BorderSoft => 6,
            Ink::Link => 7,
            Ink::Thumb => 8,
            Ink::ThumbHover => 9,
        };
        &self.brushes[index]
    }

    /// Builds a styled text layout for one run of text.
    fn build_layout(
        &self,
        text: &Text,
        format: &IDWriteTextFormat,
        width: f32,
    ) -> windows::core::Result<(IDWriteTextLayout, f32)> {
        unsafe {
            let layout = self.dwrite.CreateTextLayout(&text.utf16, format, width, f32::MAX)?;
            layout.SetWordWrapping(DWRITE_WORD_WRAPPING_WRAP)?;

            for span in &text.spans {
                let range = DWRITE_TEXT_RANGE { startPosition: span.start, length: span.len };
                match span.style {
                    Style::Strong => {
                        layout.SetFontWeight(DWRITE_FONT_WEIGHT_SEMI_BOLD, range)?;
                        layout.SetDrawingEffect(self.brush(Ink::Strong), range)?;
                    }
                    Style::Emphasis => layout.SetFontStyle(DWRITE_FONT_STYLE_ITALIC, range)?,
                    Style::Code => {
                        layout.SetFontFamilyName(w!("Consolas"), range)?;
                        layout.SetFontSize(CODE_SIZE, range)?;
                    }
                    Style::Link => {
                        layout.SetDrawingEffect(self.brush(Ink::Link), range)?;
                    }
                }
            }

            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            Ok((layout, metrics.height))
        }
    }

    /// Rebuilds the display list for the current viewport width.
    ///
    /// Args:
    ///     view: Client size in DIPs.
    fn relayout(&mut self, view: (f32, f32)) -> windows::core::Result<()> {
        self.view = view;
        self.items.clear();

        // The scrollbar always reserves its width so text never sits underneath
        // it, matching how a browser gutter behaves.
        let usable = (view.0 - SCROLLBAR_W).max(120.0);
        let column = usable.min(MAX_COLUMN);
        let text_width = (column - PAD_X * 2.0).max(80.0);
        let origin_x = ((usable - column) / 2.0 + PAD_X).max(0.0);
        let mut y = PAD_Y;
        let blocks = std::mem::take(&mut self.blocks);

        for (index, block) in blocks.iter().enumerate() {
            match block {
                Block::Rule => {
                    y += RULE_GAP;
                    self.items.push(Item {
                        draw: Draw::Line {
                            x0: origin_x,
                            y0: y,
                            x1: origin_x + text_width,
                            y1: y,
                            ink: Ink::BorderSoft,
                            width: 1.0,
                        },
                        top: y - 1.0,
                        bottom: y + 1.0,
                    });
                    y += RULE_GAP;
                }

                Block::Text { flavor, text, indent, quote_depth, tight } => {
                    let quote_inset = *quote_depth as f32 * (QUOTE_BAR + QUOTE_PAD);
                    let x = origin_x + indent + quote_inset;
                    let avail = (text_width - indent - quote_inset).max(60.0);

                    let gap = match flavor {
                        Flavor::Heading(_) if index > 0 => HEADING_GAP_ABOVE,
                        Flavor::Item if *tight => ITEM_GAP,
                        _ if index > 0 => BLOCK_GAP,
                        _ => 0.0,
                    };
                    y += gap;

                    let (format, ink) = match flavor {
                        Flavor::Heading(n) => {
                            (self.headings[(*n as usize - 1).min(5)].clone(), Ink::Strong)
                        }
                        Flavor::Code => (self.code.clone(), Ink::Text),
                        Flavor::Item => (self.item.clone(), Ink::Text),
                        Flavor::Body => (self.body.clone(), Ink::Text),
                    };
                    let ink = if *quote_depth > 0 { Ink::Muted } else { ink };

                    let inner = if *flavor == Flavor::Code { avail - CODE_PAD * 2.0 } else { avail };
                    let (layout, height) = self.build_layout(text, &format, inner.max(40.0))?;

                    if *flavor == Flavor::Code {
                        let rect = D2D_RECT_F {
                            left: x,
                            top: y,
                            right: x + avail,
                            bottom: y + height + CODE_PAD * 2.0,
                        };
                        self.items.push(Item {
                            draw: Draw::Rect { rect, ink: Ink::Surface, radius: 6.0 },
                            top: rect.top,
                            bottom: rect.bottom,
                        });
                        self.items.push(Item {
                            draw: Draw::Text {
                                layout,
                                x: x + CODE_PAD,
                                y: y + CODE_PAD,
                                ink,
                            },
                            top: y,
                            bottom: y + height + CODE_PAD * 2.0,
                        });
                        y += height + CODE_PAD * 2.0;
                    } else {
                        for depth in 0..*quote_depth {
                            let bar_x = origin_x + indent + depth as f32 * (QUOTE_BAR + QUOTE_PAD);
                            let rect = D2D_RECT_F {
                                left: bar_x,
                                top: y,
                                right: bar_x + QUOTE_BAR,
                                bottom: y + height,
                            };
                            self.items.push(Item {
                                draw: Draw::Rect { rect, ink: Ink::Border, radius: 0.0 },
                                top: rect.top,
                                bottom: rect.bottom,
                            });
                        }
                        self.items.push(Item {
                            draw: Draw::Text { layout, x, y, ink },
                            top: y,
                            bottom: y + height,
                        });
                        y += height;

                        // h1 and h2 carry a bottom rule in the CSS.
                        if let Flavor::Heading(n) = flavor {
                            if *n <= 2 {
                                y += 0.3 * if *n == 1 { 32.0 } else { 24.0 };
                                self.items.push(Item {
                                    draw: Draw::Line {
                                        x0: x,
                                        y0: y,
                                        x1: origin_x + text_width,
                                        y1: y,
                                        ink: Ink::BorderSoft,
                                        width: 1.0,
                                    },
                                    top: y - 1.0,
                                    bottom: y + 1.0,
                                });
                            }
                        }
                    }
                }

                Block::Table(rows) => {
                    y += BLOCK_GAP;
                    let columns = rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
                    if columns == 0 {
                        continue;
                    }
                    // Equal column widths. Real auto-width table layout is not
                    // implemented in this beta.
                    let col_width = text_width / columns as f32;
                    let cell_width = (col_width - CELL_PAD_X * 2.0).max(30.0);
                    let table_top = y;

                    for row in rows {
                        let mut laid = Vec::new();
                        let mut row_height: f32 = 0.0;
                        for cell in &row.cells {
                            let format =
                                if row.header { self.headings[5].clone() } else { self.body.clone() };
                            let (layout, height) = self.build_layout(cell, &format, cell_width)?;
                            row_height = row_height.max(height);
                            laid.push(layout);
                        }
                        let row_bottom = y + row_height + CELL_PAD_Y * 2.0;

                        if row.header {
                            self.items.push(Item {
                                draw: Draw::Rect {
                                    rect: D2D_RECT_F {
                                        left: origin_x,
                                        top: y,
                                        right: origin_x + text_width,
                                        bottom: row_bottom,
                                    },
                                    ink: Ink::Surface,
                                    radius: 0.0,
                                },
                                top: y,
                                bottom: row_bottom,
                            });
                        }

                        for (column, layout) in laid.into_iter().enumerate() {
                            self.items.push(Item {
                                draw: Draw::Text {
                                    layout,
                                    x: origin_x + column as f32 * col_width + CELL_PAD_X,
                                    y: y + CELL_PAD_Y,
                                    ink: if row.header { Ink::Strong } else { Ink::Text },
                                },
                                top: y,
                                bottom: row_bottom,
                            });
                        }

                        // Horizontal rule under the row.
                        self.items.push(Item {
                            draw: Draw::Line {
                                x0: origin_x,
                                y0: row_bottom,
                                x1: origin_x + text_width,
                                y1: row_bottom,
                                ink: Ink::Border,
                                width: 1.0,
                            },
                            top: row_bottom - 1.0,
                            bottom: row_bottom + 1.0,
                        });
                        y = row_bottom;
                    }

                    // Column separators and outer edges.
                    for column in 0..=columns {
                        let x = origin_x + column as f32 * col_width;
                        self.items.push(Item {
                            draw: Draw::Line {
                                x0: x,
                                y0: table_top,
                                x1: x,
                                y1: y,
                                ink: Ink::Border,
                                width: 1.0,
                            },
                            top: table_top,
                            bottom: y,
                        });
                    }
                    self.items.push(Item {
                        draw: Draw::Line {
                            x0: origin_x,
                            y0: table_top,
                            x1: origin_x + text_width,
                            y1: table_top,
                            ink: Ink::Border,
                            width: 1.0,
                        },
                        top: table_top - 1.0,
                        bottom: table_top + 1.0,
                    });
                }
            }
        }

        self.blocks = blocks;
        self.content_height = y + PAD_Y;
        self.clamp_scroll();
        Ok(())
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height - self.view.1).max(0.0)
    }

    /// Thumb geometry, or `None` when the document fits and no bar is needed.
    fn thumb(&self) -> Option<Thumb> {
        let max = self.max_scroll();
        if max <= 0.0 || self.view.1 <= 0.0 {
            return None;
        }
        let track = self.view.1;
        let visible = (track / self.content_height).clamp(0.0, 1.0);
        let height = (track * visible).max(THUMB_MIN).min(track);
        let top = (self.scroll / max) * (track - height);
        Some(Thumb { top, height })
    }

    /// True when `x` falls inside the scrollbar gutter.
    fn in_gutter(&self, x: f32) -> bool {
        x >= self.view.0 - SCROLLBAR_W
    }

    /// Recomputes hover state, reporting whether it changed.
    fn update_hover(&mut self) -> bool {
        let over = match self.thumb() {
            Some(t) => {
                self.in_gutter(self.cursor.0)
                    && self.cursor.1 >= t.top
                    && self.cursor.1 <= t.top + t.height
            }
            None => false,
        };
        let changed = over != self.hover_thumb;
        self.hover_thumb = over;
        changed
    }

    /// Begins a drag, or jumps the thumb if the click landed on bare track.
    ///
    /// Args:
    ///     y: Click position in DIPs.
    ///
    /// Returns:
    ///     True when the view needs repainting.
    fn press_gutter(&mut self, y: f32) -> bool {
        let Some(t) = self.thumb() else {
            return false;
        };
        if y >= t.top && y <= t.top + t.height {
            self.drag_grab = Some(y - t.top);
            self.hover_thumb = true;
            true
        } else {
            // Centre the thumb on the click, then continue as a drag so the
            // user can keep adjusting without releasing.
            self.drag_grab = Some(t.height / 2.0);
            self.drag_to(y);
            self.hover_thumb = true;
            true
        }
    }

    /// Maps a cursor position to a scroll offset while dragging.
    fn drag_to(&mut self, y: f32) -> bool {
        let Some(grab) = self.drag_grab else {
            return false;
        };
        let Some(t) = self.thumb() else {
            return false;
        };
        let span = self.view.1 - t.height;
        if span <= 0.0 {
            return false;
        }
        let target = ((y - grab) / span) * self.max_scroll();
        let before = self.scroll;
        self.scroll = target.clamp(0.0, self.max_scroll());
        self.scroll != before
    }

    fn clamp_scroll(&mut self) {
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    /// Scrolls by `delta` DIPs and reports whether the offset actually moved.
    fn scroll_by(&mut self, delta: f32) -> bool {
        let before = self.scroll;
        self.scroll = (self.scroll + delta).clamp(0.0, self.max_scroll());
        self.scroll != before
    }

    /// Draws the visible slice of the display list.
    fn paint(&self) -> windows::core::Result<()> {
        unsafe {
            self.target.BeginDraw();
            self.target.Clear(Some(&color(BG)));

            let top = self.scroll;
            let bottom = self.scroll + self.view.1;

            for item in &self.items {
                if item.bottom < top || item.top > bottom {
                    continue;
                }
                match &item.draw {
                    Draw::Text { layout, x, y, ink } => {
                        self.target.DrawTextLayout(
                            Vector2 { X: *x, Y: *y - self.scroll },
                            layout,
                            self.brush(*ink),
                            D2D1_DRAW_TEXT_OPTIONS_NONE,
                        );
                    }
                    Draw::Rect { rect, ink, radius } => {
                        let shifted = D2D_RECT_F {
                            left: rect.left,
                            top: rect.top - self.scroll,
                            right: rect.right,
                            bottom: rect.bottom - self.scroll,
                        };
                        if *radius > 0.0 {
                            self.target.FillRoundedRectangle(
                                &D2D1_ROUNDED_RECT {
                                    rect: shifted,
                                    radiusX: *radius,
                                    radiusY: *radius,
                                },
                                self.brush(*ink),
                            );
                        } else {
                            self.target.FillRectangle(&shifted, self.brush(*ink));
                        }
                    }
                    Draw::Line { x0, y0, x1, y1, ink, width } => {
                        self.target.DrawLine(
                            Vector2 { X: *x0, Y: *y0 - self.scroll },
                            Vector2 { X: *x1, Y: *y1 - self.scroll },
                            self.brush(*ink),
                            *width,
                            None,
                        );
                    }
                }
            }

            // Scrollbar last so it sits above content, and unculled since it is
            // in viewport space rather than document space.
            if let Some(t) = self.thumb() {
                let left = self.view.0 - SCROLLBAR_W;
                self.target.FillRectangle(
                    &D2D_RECT_F {
                        left,
                        top: 0.0,
                        right: self.view.0,
                        bottom: self.view.1,
                    },
                    self.brush(Ink::Background),
                );

                let inset = 1.0;
                let radius = (SCROLLBAR_W - inset * 2.0) / 2.0;
                let ink = if self.hover_thumb || self.drag_grab.is_some() {
                    Ink::ThumbHover
                } else {
                    Ink::Thumb
                };
                self.target.FillRoundedRectangle(
                    &D2D1_ROUNDED_RECT {
                        rect: D2D_RECT_F {
                            left: left + inset,
                            top: t.top,
                            right: self.view.0 - inset,
                            bottom: t.top + t.height,
                        },
                        radiusX: radius,
                        radiusY: radius,
                    },
                    self.brush(ink),
                );
            }

            self.target.EndDraw(None, None)?;
            Ok(())
        }
    }

    fn resize(&mut self, pixels: (u32, u32)) -> windows::core::Result<()> {
        unsafe {
            self.target.Resize(&D2D_SIZE_U {
                width: pixels.0.max(1),
                height: pixels.1.max(1),
            })?;
        }
        Ok(())
    }
}

fn color(rgb: u32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((rgb >> 16) & 0xff) as f32 / 255.0,
        g: ((rgb >> 8) & 0xff) as f32 / 255.0,
        b: (rgb & 0xff) as f32 / 255.0,
        a: 1.0,
    }
}

fn make_format(
    dwrite: &IDWriteFactory,
    family: windows::core::PCWSTR,
    size: f32,
    line: f32,
    strong: bool,
) -> windows::core::Result<IDWriteTextFormat> {
    unsafe {
        let weight = if strong { DWRITE_FONT_WEIGHT_SEMI_BOLD } else { DWRITE_FONT_WEIGHT_NORMAL };
        let format = dwrite.CreateTextFormat(
            family,
            None::<&IDWriteFontCollection>,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-us"),
        )?;
        // Uniform line spacing so leading matches the CSS line-height rather
        // than whatever the font's own metrics suggest.
        format.SetLineSpacing(DWRITE_LINE_SPACING_METHOD_UNIFORM, line, line * 0.8)?;
        Ok(format)
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Opens a window and renders `source` with Direct2D until the user closes it.
///
/// Args:
///     source: Markdown text to display.
///     title: Window title.
///     icon: Window and taskbar icon.
///     trace: Startup trace shared with `main`, so both renderers report
///         timings on the same clock.
///
/// Returns:
///     Never; the process exits when the window is closed.
pub fn run(
    source: &str,
    title: &str,
    icon: Icon,
    trace: std::rc::Rc<std::cell::RefCell<crate::StartupTrace>>,
) -> ! {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_window_icon(Some(icon))
        .with_background_color((13, 17, 23, 255))
        .with_visible(false)
        .with_inner_size(tao::dpi::LogicalSize::new(920.0, 700.0))
        .build(&event_loop)
        .expect("Failed to create window");

    let physical = window.inner_size();
    let scale = window.scale_factor();
    let logical = physical.to_logical::<f32>(scale);

    trace.borrow_mut().mark("window_created");

    let mut renderer = Renderer::new(window.hwnd(), (physical.width, physical.height))
        .expect("Failed to initialise Direct2D");
    trace.borrow_mut().mark("d2d_ready");

    renderer.blocks = parse(source);
    trace.borrow_mut().mark("markdown_parsed");

    renderer
        .relayout((logical.width, logical.height))
        .expect("Failed to lay out document");
    trace.borrow_mut().mark("laid_out");

    // Paint before the window is shown so it never appears empty.
    let _ = renderer.paint();
    window.set_visible(true);
    {
        let mut trace = trace.borrow_mut();
        trace.mark("window_shown");
        trace.flush();
    }

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                // Exit directly; see the note in main.rs on tao's exit handling.
                std::process::exit(0);
            }

            Event::WindowEvent { event: WindowEvent::Resized(size), .. } => {
                if renderer.resize((size.width, size.height)).is_ok() {
                    let logical = size.to_logical::<f32>(window.scale_factor());
                    let _ = renderer.relayout((logical.width, logical.height));
                    let _ = renderer.paint();
                }
            }

            Event::WindowEvent { event: WindowEvent::CursorMoved { position, .. }, .. } => {
                let p = position.to_logical::<f32>(window.scale_factor());
                renderer.cursor = (p.x, p.y);
                let moved = renderer.drag_to(p.y);
                let hover_changed = renderer.update_hover();
                if moved || hover_changed {
                    let _ = renderer.paint();
                }
            }

            Event::WindowEvent { event: WindowEvent::CursorLeft { .. }, .. } => {
                renderer.cursor = (-1.0, -1.0);
                if renderer.update_hover() {
                    let _ = renderer.paint();
                }
            }

            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button: MouseButton::Left, .. },
                ..
            } => match state {
                ElementState::Pressed => {
                    if renderer.in_gutter(renderer.cursor.0) {
                        let y = renderer.cursor.1;
                        if renderer.press_gutter(y) {
                            let _ = renderer.paint();
                        }
                    }
                }
                ElementState::Released => {
                    if renderer.drag_grab.take().is_some() {
                        renderer.update_hover();
                        let _ = renderer.paint();
                    }
                }
                _ => {}
            },

            Event::WindowEvent { event: WindowEvent::MouseWheel { delta, .. }, .. } => {
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => -y * BODY_LINE * 3.0,
                    MouseScrollDelta::PixelDelta(p) => -p.y as f32,
                    _ => 0.0,
                };
                if renderer.scroll_by(dy) {
                    let _ = renderer.paint();
                }
            }

            Event::WindowEvent { event: WindowEvent::KeyboardInput { event, .. }, .. } => {
                if event.state != ElementState::Pressed {
                    return;
                }
                let page = renderer.view.1 * 0.9;
                let dy = match event.physical_key {
                    KeyCode::ArrowDown => BODY_LINE * 3.0,
                    KeyCode::ArrowUp => -BODY_LINE * 3.0,
                    KeyCode::PageDown | KeyCode::Space => page,
                    KeyCode::PageUp => -page,
                    KeyCode::Home => -renderer.content_height,
                    KeyCode::End => renderer.content_height,
                    KeyCode::Escape => std::process::exit(0),
                    _ => 0.0,
                };
                if dy != 0.0 && renderer.scroll_by(dy) {
                    let _ = renderer.paint();
                }
            }

            Event::RedrawRequested(_) => {
                let _ = renderer.paint();
            }

            _ => {}
        }
    });
}
