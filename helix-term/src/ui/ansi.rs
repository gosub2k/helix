//! Minimal ANSI-SGR parser that converts an escape-laden string into a
//! `tui::text::Text<'static>` with styled spans. Supports the SGR subset
//! common in compiler/test output: basic and bright fg/bg, 256-colour,
//! truecolor, bold, dim, italic, underline, reverse, strikethrough, and
//! their "off" counterparts. Non-SGR CSI and OSC sequences are consumed
//! and discarded.

use crate::compositor::{Component, Context};
use helix_view::graphics::{Color, Modifier, Rect, Style, UnderlineStyle};
use tui::buffer::Buffer as Surface;
use tui::text::{Span, Spans, Text};
use tui::widgets::{Paragraph, Widget, Wrap};

pub fn ansi_to_text(input: &str) -> Text<'static> {
    let mut lines: Vec<Spans<'static>> = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut buf = String::new();
    let mut style = Style::default();

    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    if !buf.is_empty() {
                        current_line
                            .push(Span::styled(std::mem::take(&mut buf), style));
                    }
                    let mut params = String::new();
                    let mut terminator = '\0';
                    for nc in chars.by_ref() {
                        if nc.is_ascii_alphabetic() || nc == '~' {
                            terminator = nc;
                            break;
                        }
                        params.push(nc);
                    }
                    if terminator == 'm' {
                        apply_sgr(&mut style, &params);
                    }
                    // Other CSI final bytes (cursor moves, erase, etc.) are dropped.
                }
                Some(']') => {
                    // OSC: ESC ] ... BEL  or  ESC ] ... ESC \
                    chars.next();
                    while let Some(nc) = chars.next() {
                        if nc == '\x07' {
                            break;
                        }
                        if nc == '\x1b' {
                            let _ = chars.next();
                            break;
                        }
                    }
                }
                _ => {
                    // Other escapes: swallow the one following byte.
                    chars.next();
                }
            },
            '\n' => {
                if !buf.is_empty() {
                    current_line
                        .push(Span::styled(std::mem::take(&mut buf), style));
                }
                lines.push(Spans(std::mem::take(&mut current_line)));
            }
            '\r' => {}
            c => buf.push(c),
        }
    }
    if !buf.is_empty() {
        current_line.push(Span::styled(buf, style));
    }
    if !current_line.is_empty() {
        lines.push(Spans(current_line));
    }

    Text { lines }
}

fn apply_sgr(style: &mut Style, params: &str) {
    let codes: Vec<u32> = if params.is_empty() {
        vec![0]
    } else {
        params.split(';').filter_map(|p| p.parse().ok()).collect()
    };

    let mut i = 0;
    while i < codes.len() {
        let code = codes[i];
        match code {
            0 => *style = Style::default(),
            1 => *style = style.add_modifier(Modifier::BOLD),
            2 => *style = style.add_modifier(Modifier::DIM),
            3 => *style = style.add_modifier(Modifier::ITALIC),
            4 => *style = style.underline_style(UnderlineStyle::Line),
            7 => *style = style.add_modifier(Modifier::REVERSED),
            9 => *style = style.add_modifier(Modifier::CROSSED_OUT),
            22 => *style = style.remove_modifier(Modifier::BOLD | Modifier::DIM),
            23 => *style = style.remove_modifier(Modifier::ITALIC),
            24 => style.underline_style = None,
            27 => *style = style.remove_modifier(Modifier::REVERSED),
            29 => *style = style.remove_modifier(Modifier::CROSSED_OUT),
            30..=37 => *style = style.fg(basic_color(code - 30)),
            38 => {
                if let Some((c, consumed)) = extended_color(&codes[i + 1..]) {
                    *style = style.fg(c);
                    i += consumed;
                }
            }
            39 => *style = style.fg(Color::Reset),
            40..=47 => *style = style.bg(basic_color(code - 40)),
            48 => {
                if let Some((c, consumed)) = extended_color(&codes[i + 1..]) {
                    *style = style.bg(c);
                    i += consumed;
                }
            }
            49 => *style = style.bg(Color::Reset),
            90..=97 => *style = style.fg(bright_color(code - 90)),
            100..=107 => *style = style.bg(bright_color(code - 100)),
            _ => {}
        }
        i += 1;
    }
}

fn basic_color(n: u32) -> Color {
    match n {
        0 => Color::Black,
        1 => Color::Red,
        2 => Color::Green,
        3 => Color::Yellow,
        4 => Color::Blue,
        5 => Color::Magenta,
        6 => Color::Cyan,
        7 => Color::Gray,
        _ => Color::Reset,
    }
}

fn bright_color(n: u32) -> Color {
    match n {
        0 => Color::Black,
        1 => Color::LightRed,
        2 => Color::LightGreen,
        3 => Color::LightYellow,
        4 => Color::LightBlue,
        5 => Color::LightMagenta,
        6 => Color::LightCyan,
        7 => Color::White,
        _ => Color::Reset,
    }
}

fn extended_color(rest: &[u32]) -> Option<(Color, usize)> {
    match rest.first()? {
        5 => {
            let idx = *rest.get(1)? as u8;
            Some((Color::Indexed(idx), 2))
        }
        2 => {
            let r = *rest.get(1)? as u8;
            let g = *rest.get(2)? as u8;
            let b = *rest.get(3)? as u8;
            Some((Color::Rgb(r, g, b), 4))
        }
        _ => None,
    }
}

/// Component that renders ANSI-coloured output inside a scrollable popup.
/// Mirrors the `Markdown` component's scroll/required-size contract so it
/// plugs into the Popup widget without further work.
pub struct AnsiText {
    text: Text<'static>,
}

impl AnsiText {
    pub fn new(raw: &str) -> Self {
        Self {
            text: ansi_to_text(raw),
        }
    }
}

impl Component for AnsiText {
    fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        let par = Paragraph::new(&self.text)
            .wrap(Wrap { trim: false })
            .scroll((cx.scroll.unwrap_or_default() as u16, 0));
        par.render(area, surface);
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        // Width of the widest line (clamped to viewport) determines unwrapped
        // horizontal extent; Paragraph wraps so vertical extent grows for
        // any line that exceeds the wrap width.
        let width = self
            .text
            .lines
            .iter()
            .map(|l| l.width() as u16)
            .max()
            .unwrap_or(0)
            .min(viewport.0);
        let wrap_width = viewport.0.max(1);
        let mut height: u16 = 0;
        for line in &self.text.lines {
            let w = line.width() as u16;
            height = height.saturating_add(if w == 0 { 1 } else { w.div_ceil(wrap_width) });
        }
        Some((width, height))
    }
}
