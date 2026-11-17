//! Settings › Remote Hosts: each host from hosts.json with its endpoint, its state in the main
//! window (connected, or why not) and its tailnet login; Add Host… (the main window's prompt,
//! Tailscale's machines included), Rename and Remove (which asks first).

use gpui::{PromptButton, PromptLevel};

use super::patterns::{typed, Typed};
use super::server::text_button;
use super::*;
use crate::hosts::HostLine;

/// The Rename field: the host it renames, what is typed, and why the last try was refused.
pub(super) struct RenameField {
    focus: FocusHandle,
    host: Option<String>,
    draft: String,
    error: Option<String>,
}

impl RenameField {
    pub(super) fn new(cx: &mut Context<SettingsWindow>) -> Self {
        RenameField { focus: cx.focus_handle(), host: None, draft: String::new(), error: None }
    }
}

impl SettingsWindow {
    /// The hosts as the page lists them (from the main window).
    pub fn host_lines(&self) -> Vec<HostLine> {
        self.hosts.clone()
    }

    /// Why the last rename was refused.
    pub fn rename_error(&self) -> Option<String> {
        self.rename.error.clone()
    }

    /// Follow the main window: its hosts as a snapshot, taken again on each of its changes, which
    /// redraws this window only when the hosts changed (R4); and a Restart waiting for it.
    pub(super) fn follow_workspace(&mut self, cx: &mut Context<Self>) {
        let Some(ws) = Self::workspace(cx).and_then(|w| w.update(cx, |_, _, cx| cx.entity()).ok()) else { return };
        self.hosts = ws.read(cx).host_lines();
        self._workspace = Some(cx.observe(&ws, |sw, ws, cx| {
            sw.restart_when_stopped(cx);
            let lines = ws.read(cx).host_lines();
            if lines != sw.hosts {
                sw.hosts = lines;
                cx.notify();
            }
        }));
        cx.notify();
    }

    /// Ask, then forget the host `label` (hosts.json and the main window).
    fn confirm_remove(&mut self, label: String, window: &mut Window, cx: &mut Context<Self>) {
        let detail = "leon stops showing it and forgets its address. Its server and the programs in its \
                      sessions keep running on that machine.";
        let buttons = [PromptButton::Ok("Remove".into()), PromptButton::Cancel("Cancel".into())];
        let answer = window.prompt(PromptLevel::Warning, &format!("Remove {label}?"), Some(detail), &buttons, cx);
        cx.spawn(async move |this, cx| {
            if answer.await != Ok(0) {
                return;
            }
            let _ = this.update(cx, |sw, cx| {
                let done = Self::workspace(cx).map(|w| w.update(cx, |ws, _, cx| ws.remove_host(&label, cx)));
                if let Some(Ok(Err(e))) = done {
                    sw.error = Some(format!("can't remove {label}: {e}"));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn rename_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match typed(&mut self.rename.draft, e, cx) {
            Typed::Enter => {
                let (Some(old), new) = (self.rename.host.clone(), self.rename.draft.trim().to_string()) else { return };
                let done = Self::workspace(cx).map(|w| w.update(cx, |ws, _, cx| ws.rename_host(&old, &new, cx)));
                match done {
                    Some(Ok(Ok(()))) => {
                        let f = &mut self.rename;
                        (f.host, f.draft, f.error) = (None, String::new(), None);
                        window.focus(&self.focus);
                    }
                    Some(Ok(Err(e))) => self.rename.error = Some(e),
                    _ => self.rename.error = Some("the main window is closed".into()),
                }
                cx.notify();
            }
            Typed::Escape => {
                let f = &mut self.rename;
                (f.host, f.draft, f.error) = (None, String::new(), None);
                window.focus(&self.focus);
            }
            Typed::Edited | Typed::Other => {}
        }
    }

    pub(super) fn hosts_intro(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let has = Self::workspace(cx).is_some();
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .child(Self::label(
                t,
                "Other machines' leon servers, reached over ssh",
                if !has {
                    "Open leon's main window to see and change them"
                } else if self.hosts.is_empty() {
                    "None yet: add one here, or with leon hosts add in a terminal"
                } else {
                    "The hosts list leon hosts shows (docs/remote-hosts.md)"
                },
                false,
            ))
            .when(has, |d| {
                d.child(text_button(t, "hosts-add".into(), "Add Host…", cx, |_, _, cx| {
                    if let Some(w) = Self::workspace(cx) {
                        let _ = w.update(cx, |ws, window, cx| ws.add_host(window, cx));
                    }
                }))
            })
    }

    /// The hosts at `rows` (indexes into `hosts`): label, endpoint, state, tailnet login, and
    /// Rename and Remove.
    pub(super) fn host_rows(&self, rows: &[usize], t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let mut out = div().flex().flex_col().gap(px(14.));
        for &i in rows {
            let Some(h) = self.hosts.get(i) else { continue };
            let (label, renaming) = (h.label.clone(), self.rename.host.as_deref() == Some(h.label.as_str()));
            let state = match (&h.note, h.connected) {
                (_, true) => div().text_color(rgb(t.done)).child("Connected"),
                (Some(n), _) => div().text_color(rgb(t.error)).child(n.clone()),
                (None, _) => div().text_color(rgb(t.muted)).child("Not connected"),
            };
            let mut row = div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .child(div().text_color(rgb(t.title)).child(h.label.clone()))
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(text_button(t, format!("host-rename-{i}"), "Rename", cx, {
                                    let label = label.clone();
                                    move |sw, window, cx| {
                                        let f = &mut sw.rename;
                                        (f.host, f.draft, f.error) = (Some(label.clone()), label.clone(), None);
                                        window.focus(&sw.rename.focus);
                                        cx.notify();
                                    }
                                }))
                                .child(text_button(t, format!("host-remove-{i}"), "Remove…", cx, {
                                    let label = label.clone();
                                    move |sw, window, cx| sw.confirm_remove(label.clone(), window, cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .font_family("Menlo")
                        .text_size(px(SECONDARY_TEXT))
                        .text_color(rgb(t.muted))
                        .child(h.endpoint.clone()),
                )
                .child(state.text_size(px(SECONDARY_TEXT)))
                .children(
                    h.tailnet.clone().map(|l| div().text_size(px(SECONDARY_TEXT)).text_color(rgb(t.muted)).child(l)),
                );
            if renaming {
                let f = &self.rename;
                row = row
                    .child(
                        div()
                            .id("host-rename-field")
                            .debug_selector(|| "host-rename-field".into())
                            .track_focus(&f.focus)
                            .on_key_down(cx.listener(Self::rename_key))
                            .px(px(8.))
                            .py(px(4.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(t.accent))
                            .bg(rgb(t.bg))
                            .child(format!("{}▏", f.draft)),
                    )
                    .children(f.error.clone().map(|e| {
                        div()
                            .debug_selector(|| "host-rename-error".into())
                            .text_size(px(SECONDARY_TEXT))
                            .text_color(rgb(t.error))
                            .child(e)
                    }));
            }
            out = out.child(row);
        }
        out
    }
}
