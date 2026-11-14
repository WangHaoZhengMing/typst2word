//! Native macOS Sequoia / Sonoma theme configuration and semantic status helpers.
//!
//! Uses distinct macOS material layers (`sidebar` source list, `title_bar` unified chrome,
//! `muted` neutral Preview canvas backdrop, and `background` inspector surfaces) so the
//! three-column layout reads effortlessly in both Light and Dark appearances.

use std::rc::Rc;

use gpui_kit::{
    App, Hsla, ParentElement as _, SharedString, Window,
    component::{
        ActiveTheme as _, Sizable as _,
        {Theme, ThemeMode, ThemeRegistry},
        tag::Tag,
    },
};

use crate::model::CaseStatus;

const MACOS_LIGHT_NAME: &str = "macOS Studio Light";
const MACOS_DARK_NAME: &str = "macOS Studio Dark";

const MACOS_STUDIO_THEME_JSON: &str = r##"{
  "name": "macOS Studio",
  "author": "typst2word",
  "themes": [
    {
      "name": "macOS Studio Light",
      "mode": "light",
      "shadow": true,
      "colors": {
        "background": "#FFFFFF",
        "foreground": "#1D1D1F",
        "border": "#DCDCE1",
        "ring": "#007AFF",
        "primary.background": "#007AFF",
        "primary.foreground": "#FFFFFF",
        "secondary.background": "#ECECF1",
        "secondary.hover.background": "#E2E2E9",
        "secondary.active.background": "#D6D6DF",
        "secondary.foreground": "#1D1D1F",
        "accent.background": "#007AFF1A",
        "accent.foreground": "#0062CC",
        "muted.background": "#EAECF0",
        "muted.foreground": "#6E6E73",
        "popover.background": "#FBFBFD",
        "popover.foreground": "#1D1D1F",
        "sidebar.background": "#F2F2F6",
        "sidebar.foreground": "#1D1D1F",
        "sidebar.border": "#DCDCE1",
        "title_bar.background": "#F5F5F8",
        "title_bar.border": "#DCDCE1",
        "status_bar.background": "#F5F5F8",
        "status_bar.border": "#DCDCE1",
        "tab_bar.background": "#ECECF1",
        "tab.background": "#ECECF1",
        "tab.active.background": "#FFFFFF",
        "tab.foreground": "#6E6E73",
        "tab.active.foreground": "#1D1D1F",
        "list.active.background": "#007AFF20",
        "list.active.border": "#007AFF",
        "list.hover.background": "#00000009",
        "list.even.background": "#FAFAFC",
        "scrollbar.background": "#FFFFFF00",
        "scrollbar.thumb.background": "#C4C4C9",
        "base.blue": "#007AFF",
        "base.green": "#248A3D",
        "base.yellow": "#B25000",
        "base.red": "#D70015",
        "base.magenta": "#AF52DE",
        "base.cyan": "#0071A4"
      },
      "highlight": {
        "editor.foreground": "#1D1D1F",
        "editor.background": "#FFFFFF",
        "editor.active_line.background": "#F5F7FA",
        "editor.line_number": "#98989D",
        "editor.active_line_number": "#1D1D1F",
        "editor.invisible": "#007AFF44",
        "syntax": {
          "attribute": { "color": "#815F03" },
          "boolean": { "color": "#9B2393", "font_weight": 600 },
          "comment": { "color": "#6C7986", "font_style": "italic" },
          "comment.doc": { "color": "#5D6C79", "font_style": "italic" },
          "constant": { "color": "#9B2393" },
          "constructor": { "color": "#1C464A" },
          "function": { "color": "#326D74" },
          "keyword": { "color": "#9B2393", "font_weight": 600 },
          "number": { "color": "#1C00CF" },
          "operator": { "color": "#393A34" },
          "property": { "color": "#0F68A0" },
          "punctuation": { "color": "#6E6E73" },
          "punctuation.bracket": { "color": "#48484A" },
          "punctuation.delimiter": { "color": "#6E6E73" },
          "string": { "color": "#C41A16" },
          "string.escape": { "color": "#815F03" },
          "string.regex": { "color": "#C41A16" },
          "string.special": { "color": "#0B4F79" },
          "tag": { "color": "#005CC5" },
          "text.literal": { "color": "#6F42C1" },
          "title": { "color": "#005CC5", "font_weight": 600 },
          "type": { "color": "#5C2699" },
          "variable": { "color": "#1D1D1F" },
          "variable.special": { "color": "#9B2393" }
        }
      }
    },
    {
      "name": "macOS Studio Dark",
      "mode": "dark",
      "shadow": true,
      "colors": {
        "background": "#1D1D20",
        "foreground": "#EBEBF0",
        "border": "#303036",
        "ring": "#0A84FF",
        "primary.background": "#0A84FF",
        "primary.foreground": "#FFFFFF",
        "secondary.background": "#2B2B30",
        "secondary.hover.background": "#37373E",
        "secondary.active.background": "#44444C",
        "secondary.foreground": "#EBEBF0",
        "accent.background": "#0A84FF29",
        "accent.foreground": "#64D2FF",
        "muted.background": "#141417",
        "muted.foreground": "#98989F",
        "popover.background": "#26262B",
        "popover.foreground": "#EBEBF0",
        "sidebar.background": "#18181B",
        "sidebar.foreground": "#E5E5EA",
        "sidebar.border": "#2C2C31",
        "title_bar.background": "#222226",
        "title_bar.border": "#303036",
        "status_bar.background": "#202024",
        "status_bar.border": "#303036",
        "tab_bar.background": "#28282D",
        "tab.background": "#28282D",
        "tab.active.background": "#1D1D20",
        "tab.foreground": "#98989F",
        "tab.active.foreground": "#F2F2F7",
        "list.active.background": "#0A84FF33",
        "list.active.border": "#0A84FF",
        "list.hover.background": "#FFFFFF0B",
        "list.even.background": "#212125",
        "selection.background": "#264F78",
        "scrollbar.background": "#00000000",
        "scrollbar.thumb.background": "#636366",
        "base.blue": "#0A84FF",
        "base.green": "#30D158",
        "base.yellow": "#FFD60A",
        "base.red": "#FF453A",
        "base.magenta": "#BF5AF2",
        "base.cyan": "#64D2FF"
      },
      "highlight": {
        "editor.foreground": "#EBEBF0",
        "editor.background": "#1A1A1D",
        "editor.active_line.background": "#25252A",
        "editor.line_number": "#636368",
        "editor.active_line_number": "#D1D1D6",
        "editor.invisible": "#0A84FF55",
        "syntax": {
          "attribute": { "color": "#D5B778" },
          "boolean": { "color": "#FC5FA3", "font_weight": 600 },
          "comment": { "color": "#7F8C98", "font_style": "italic" },
          "comment.doc": { "color": "#8E9BA8", "font_style": "italic" },
          "constant": { "color": "#D0BF69" },
          "constructor": { "color": "#5DD8FF" },
          "function": { "color": "#67B7A4" },
          "keyword": { "color": "#FC5FA3", "font_weight": 600 },
          "number": { "color": "#D0BF69" },
          "operator": { "color": "#A1A1A6" },
          "property": { "color": "#78C2B3" },
          "punctuation": { "color": "#8E8E93" },
          "punctuation.bracket": { "color": "#AEAEB2" },
          "punctuation.delimiter": { "color": "#8E8E93" },
          "string": { "color": "#FC6A5D" },
          "string.escape": { "color": "#D5B778" },
          "string.regex": { "color": "#FC6A5D" },
          "string.special": { "color": "#64D2FF" },
          "tag": { "color": "#5DD8FF" },
          "text.literal": { "color": "#BF5AF2" },
          "title": { "color": "#5DD8FF", "font_weight": 600 },
          "type": { "color": "#D0A8FF" },
          "variable": { "color": "#EBEBF0" },
          "variable.special": { "color": "#FC5FA3" }
        }
      }
    }
  ]
}"##;

/// Register the macOS Studio Light & Dark themes and sync with macOS appearance.
pub fn install_macos_theme(window: Option<&mut Window>, cx: &mut App) {
    let registry = ThemeRegistry::global_mut(cx);
    let _ = registry.load_themes_from_str(MACOS_STUDIO_THEME_JSON);

    let light_cfg = registry
        .themes()
        .get(&SharedString::from(MACOS_LIGHT_NAME))
        .cloned();
    let dark_cfg = registry
        .themes()
        .get(&SharedString::from(MACOS_DARK_NAME))
        .cloned();

    {
        let theme = Theme::global_mut(cx);
        if let Some(light) = light_cfg {
            theme.light_theme = Rc::clone(&light);
        }
        if let Some(dark) = dark_cfg {
            theme.dark_theme = Rc::clone(&dark);
        }
        theme.font_size = gpui_kit::px(13.5);
        theme.mono_font_size = gpui_kit::px(12.0);
        theme.radius = gpui_kit::px(6.0);
        theme.radius_lg = gpui_kit::px(8.0);
    }

    Theme::sync_system_appearance(window, cx);
    Theme::sync_scrollbar_appearance(cx);
}

/// Toggle between macOS Light and Dark appearance modes.
pub fn toggle_theme_mode(window: &mut Window, cx: &mut App) -> ThemeMode {
    let next = if cx.theme().is_dark() {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    };
    Theme::change(next, Some(window), cx);
    next
}

/// Semantic indicator dot color for a `CaseStatus` resolved through `cx.theme()`.
pub fn status_dot_color(status: CaseStatus, cx: &App) -> Hsla {
    let t = cx.theme();
    match status {
        CaseStatus::Pass => t.success,
        CaseStatus::Drift => t.warning,
        CaseStatus::Fail => t.danger,
        CaseStatus::IrOnly => t.info,
    }
}

/// Compact semantic `Tag` badge for a `CaseStatus`.
pub fn status_tag(status: CaseStatus) -> Tag {
    let tag = match status {
        CaseStatus::Pass => Tag::success(),
        CaseStatus::Drift => Tag::warning(),
        CaseStatus::Fail => Tag::danger(),
        CaseStatus::IrOnly => Tag::info(),
    };
    tag.small().outline().child(status.label())
}
