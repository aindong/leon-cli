//! The Settings window's hint patterns: an editable list, each pattern checked before it is
//! written.

use leon_core::hints::{compile, MAX_PATTERNS};

use super::*;

/// A one-line text field: the hint pattern field, and Tailscale's port and tag fields.
pub(super) struct TextField {
    pub(super) focus: FocusHandle,
    pub(super) draft: String,
    /// The hint pattern the draft replaces (by its text: init.lua may be edited meanwhile); None
    /// adds one.
    editing: Option<String>,
    /// Why the last Enter was refused.
    pub(super) error: Option<String>,
    /// The field had the focus when the window last rendered.
    pub(super) typing: bool,
}

impl TextField {
    pub(super) fn new(cx: &mut Context<SettingsWindow>) -> Self {
        TextField { focus: cx.focus_handle(), draft: String::new(), editing: None, error: None, typing: false }
    }

    pub(super) fn focused(&self, window: &Window) -> bool {
        self.focus.is_focused(window)
    }

    /// Empty the field and give the focus back to the window.
    pub(super) fn escape(&mut self, window: &mut Window, to: &FocusHandle) {
        (self.draft, self.editing, self.error) = (String::new(), None, None);
        window.focus(to);
    }

    /// The field (`{id}-field`, showing `placeholder` while empty), its `button` (`{id}-add`) and,
    /// under them, why the last Enter was refused (`{id}-error`).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn draw(
        &self,
        t: &Theme,
        id: &str,
        placeholder: String,
        button: &'static str,
        key: impl Fn(&mut SettingsWindow, &KeyDownEvent, &mut Window, &mut Context<SettingsWindow>) + 'static,
        enter: impl Fn(&mut SettingsWindow, &mut Context<SettingsWindow>) + 'static,
        cx: &mut Context<SettingsWindow>,
    ) -> gpui::Div {
        let shown = if self.draft.is_empty() && !self.typing {
            div().text_color(rgb(t.muted)).child(placeholder)
        } else {
            div().child(format!("{}{}", self.draft, if self.typing { "▏" } else { "" }))
        };
        let (field, add, error) = (format!("{id}-field"), format!("{id}-add"), format!("{id}-error"));
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id(SharedString::from(field.clone()))
                            .debug_selector(|| field)
                            .track_focus(&self.focus)
                            .on_key_down(cx.listener(key))
                            .flex_1()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(if self.typing { t.accent } else { t.border }))
                            .bg(rgb(t.bg))
                            .font_family("Menlo")
                            .cursor_text()
                            .child(shown),
                    )
                    .child(
                        div()
                            .id(SharedString::from(add.clone()))
                            .debug_selector(|| add)
                            .px(px(10.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(t.border))
                            .cursor_pointer()
                            .hover(|d| d.bg(rgb(t.hover)))
                            .on_click(cx.listener(move |sw, _, _, cx| enter(sw, cx)))
                            .child(button),
                    ),
            )
            .children(self.error.clone().map(|why| {
                div().debug_selector(|| error).text_size(px(SECONDARY_TEXT)).text_color(rgb(t.error)).child(why)
            }))
    }
}

/// What a key did in a one-line text field.
pub(super) enum Typed {
    Enter,
    Escape,
    Edited,
    /// Nothing (⌘ keys go on to the window: ⌘W).
    Other,
}

/// A key in a one-line text field holding `draft`. Every key but a ⌘ one stays in the field: ↑/↓
/// on the window would change the section under it.
pub(super) fn typed(draft: &mut String, e: &KeyDownEvent, cx: &mut Context<SettingsWindow>) -> Typed {
    let k = &e.keystroke;
    let m = &k.modifiers;
    if m.platform {
        return Typed::Other;
    }
    cx.stop_propagation();
    let did = match k.key.as_str() {
        "enter" => Typed::Enter,
        "escape" => Typed::Escape,
        "backspace" => {
            draft.pop();
            Typed::Edited
        }
        "space" => {
            draft.push(' ');
            Typed::Edited
        }
        _ => match k.key_char.as_deref().filter(|c| !c.is_empty() && !m.control && !m.alt) {
            Some(t) => {
                draft.extend(t.chars().filter(|c| !c.is_control()));
                Typed::Edited
            }
            None => Typed::Other,
        },
    };
    if !matches!(did, Typed::Other) {
        cx.notify();
    }
    did
}

impl SettingsWindow {
    /// Why the last hint pattern wasn't added.
    pub fn pattern_error(&self) -> Option<String> {
        self.pattern.error.clone()
    }

    /// The hint pattern list: each pattern (click to edit it, × to remove it) and a field to add one.
    pub(super) fn patterns(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let row = self.row("hint_patterns");
        let locked = Self::is_locked(&row);
        let list: Vec<String> = row
            .as_ref()
            .and_then(|r| r.value.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|p| p.as_str().map(String::from))
            .collect();
        let note = "Regular expressions hints (⌘⇧H) label too, besides URLs, paths and hashes; at most 32";
        let mut out = div().flex().flex_col().gap(px(6.)).child(Self::label(t, "Hint patterns", note, locked));
        for (i, p) in list.iter().enumerate() {
            let (sel, editing) = (format!("pattern-{i}"), self.pattern.editing.as_ref() == Some(p));
            let text = p.clone();
            out = out.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .id(SharedString::from(sel.clone()))
                            .debug_selector(|| sel)
                            .flex_1()
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .font_family("Menlo")
                            .bg(rgb(if editing { t.selected } else { t.bg }))
                            .when(!locked, |d| {
                                d.cursor_pointer().on_click(cx.listener(move |sw, _, window, cx| {
                                    sw.pattern.draft = text.clone();
                                    sw.pattern.editing = Some(text.clone());
                                    sw.pattern.error = None;
                                    window.focus(&sw.pattern.focus);
                                    cx.notify();
                                }))
                            })
                            .child(p.clone()),
                    )
                    .child(rows::button(t, format!("pattern-remove-{i}"), "×", !locked, cx, {
                        let list = list.clone();
                        move |sw, cx| {
                            let mut rest = list.clone();
                            let gone = rest.remove(i);
                            if sw.pattern.editing.as_ref() == Some(&gone) {
                                (sw.pattern.draft, sw.pattern.editing) = (String::new(), None);
                            }
                            sw.choose("hint_patterns", json!(rest), cx);
                        }
                    })),
            );
        }
        if !locked {
            let f = &self.pattern;
            let save = if f.editing.is_some() { "Save" } else { "Add" };
            let hint = "Add a pattern, such as [A-Z]+-\\d+".to_string();
            out = out.child(f.draw(t, "pattern", hint, save, Self::pattern_key, |sw, cx| sw.add_pattern(cx), cx));
        }
        out.children(Self::locked(t, "hint_patterns", &row))
    }

    /// A key in the hint pattern field. ⌘ keys go on to the window (⌘W).
    fn pattern_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match typed(&mut self.pattern.draft, e, cx) {
            Typed::Enter => self.add_pattern(cx),
            Typed::Escape => self.pattern.escape(window, &self.focus),
            Typed::Edited | Typed::Other => {}
        }
    }

    /// Add the field's pattern (or save the one it edits), once it compiles and the list has room;
    /// otherwise say why under the field and write nothing.
    fn add_pattern(&mut self, cx: &mut Context<Self>) {
        let Some(row) = self.row("hint_patterns").filter(|r| r.read_only.is_none()) else { return };
        let mut list: Vec<Value> = row.value.as_array().cloned().unwrap_or_default();
        let f = &mut self.pattern;
        let draft = f.draft.clone();
        if let Err(why) = compile(&draft) {
            f.error = Some(why);
            return cx.notify();
        }
        // the pattern edited, where it is now; gone from init.lua meanwhile, the draft is added
        match f.editing.as_ref().and_then(|old| list.iter().position(|p| p == old)) {
            Some(i) => list[i] = json!(draft),
            None if list.len() >= MAX_PATTERNS => {
                f.error = Some(format!("at most {MAX_PATTERNS} hint patterns; remove one first"));
                return cx.notify();
            }
            None => list.push(json!(draft)),
        }
        (f.draft, f.editing, f.error) = (String::new(), None, None);
        self.choose("hint_patterns", Value::Array(list), cx);
    }
}
