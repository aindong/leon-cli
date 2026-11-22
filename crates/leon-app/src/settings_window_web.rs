//! The Tailscale section's Web page part (docs/web.md): its switch, the offer to turn on
//! Tailscale access when it is off, and while the page is served its address with Copy, a QR code
//! to open it on a phone, and the other addresses; the HTTPS files under a disclosure.

use gpui::{canvas, fill, ClipboardItem};

use super::server::text_button;
use super::*;
use crate::qr;

/// The QR code's side in points, quiet zone included: big enough for a phone to read.
const QR_SIDE: f32 = 160.;
pub const QR_NOTE: &str = "Open on a phone or another machine on your Tailscale; this Mac's own browser is refused.";

/// What the Web page part says, from the two switches and `server.status`'s `tailnet`.
#[derive(Debug, Clone, PartialEq)]
pub enum WebState {
    /// `serve_web` is off: the switch alone.
    Off,
    /// `serve_web` is on but `serve_tailscale`, which serves it, is off.
    NeedsTailscale,
    /// Tailscale access is on but not serving: why.
    NotServing(String),
    /// Served: its URLs (the MagicDNS name first), and why HTTPS serves nothing, if it doesn't.
    Serving { urls: Vec<String>, error: Option<String> },
    /// Nothing to say yet: no status, or no address yet.
    Waiting,
}

pub fn web_state(serve_web: bool, serve_tailscale: bool, tailnet: Option<&Value>) -> WebState {
    let text = |v: &Value| v.as_str().unwrap_or("").to_string();
    match (serve_web, serve_tailscale, tailnet) {
        (false, ..) => WebState::Off,
        (true, false, _) => WebState::NeedsTailscale,
        (true, true, Some(t)) if t["state"] == "stopped" => WebState::NotServing(text(&t["reason"])),
        (true, true, Some(t)) if t["state"] == "serving" => {
            let urls: Vec<String> = t["web"].as_array().into_iter().flatten().map(text).collect();
            let error = t["web_error"].as_str().map(String::from);
            match urls.is_empty() && error.is_none() {
                true => WebState::Waiting,
                false => WebState::Serving { urls, error },
            }
        }
        _ => WebState::Waiting,
    }
}

/// The section's disclosures: closed until clicked.
#[derive(Default)]
pub(super) struct Open {
    pub advanced: bool,
    pub others: bool,
    /// The `tailscale` the cert command names while "HTTPS (optional)" is open, looked up when it
    /// opened (render mustn't); the command itself follows the files set.
    pub https: Option<String>,
}

/// `url`'s host when it is a name (not an address), for `tailscale cert`.
fn host_name(url: &str) -> Option<String> {
    let host = url.split("://").nth(1)?.split(['/', ':']).next()?;
    // a DNS name only: it goes into a command to paste unquoted
    let dns = host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    (!host.is_empty() && dns && host.parse::<std::net::IpAddr>().is_err()).then(|| host.into())
}

impl SettingsWindow {
    /// Show `tailnet` as if `server.status` had said it (tests and screenshots).
    pub fn show_tailnet(&mut self, tailnet: Value, cx: &mut Context<Self>) {
        self.tailnet = Some(tailnet);
        cx.notify();
    }

    fn on(&self, key: &str) -> bool {
        self.row(key).is_some_and(|r| r.value == true)
    }

    /// What the Web page part says now.
    pub fn web(&self) -> WebState {
        web_state(self.on("serve_web"), self.on("serve_tailscale"), self.tailnet.as_ref())
    }

    /// The address the page is served at first (the MagicDNS name's), while served.
    pub fn web_url(&self) -> Option<String> {
        match self.web() {
            WebState::Serving { urls, .. } => urls.into_iter().next(),
            _ => None,
        }
    }

    pub fn web_note(&self) -> &'static str {
        QR_NOTE
    }

    /// The command that makes the HTTPS files, for the files set (or the usual place) and this
    /// machine's name.
    pub fn cert_hint(&self) -> String {
        let file = |key: &str, usual: &str| {
            let set = self.row(key).and_then(|r| r.value.as_str().map(String::from)).filter(|p| !p.is_empty());
            let p = set.unwrap_or_else(|| usual.into());
            match (p.strip_prefix("~/"), std::env::var_os("HOME")) {
                (Some(rest), Some(home)) => Path::new(&home).join(rest),
                _ => PathBuf::from(p),
            }
        };
        let name = self.web_url().and_then(|u| host_name(&u)).unwrap_or_else(|| "MACHINE.TAILNET.ts.net".into());
        let ts = self.open.https.clone().unwrap_or_else(leon_hosts::tailscale::cert_tool);
        leon_hosts::tailscale::cert_command(
            &ts,
            &file("serve_web_cert", "~/.config/leon/certs/web.crt"),
            &file("serve_web_key", "~/.config/leon/certs/web.key"),
            &name,
        )
    }

    /// A row that opens or closes what follows it.
    pub(super) fn disclosure(
        t: &Theme,
        id: &'static str,
        label: &str,
        open: bool,
        cx: &mut Context<Self>,
        flip: impl Fn(&mut Self) + 'static,
    ) -> impl IntoElement {
        div()
            .id(id)
            .debug_selector(|| id.into())
            .cursor_pointer()
            .text_color(rgb(t.muted))
            .hover(|d| d.text_color(rgb(t.text)))
            .on_click(cx.listener(move |sw, _, _, cx| {
                flip(sw);
                cx.notify();
            }))
            .child(format!("{} {label}", if open { "▾" } else { "▸" }))
    }

    pub(super) fn web_page(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let note = |s: String, color: u32, sel: &'static str| {
            div().debug_selector(|| sel.into()).text_size(px(SECONDARY_TEXT)).text_color(rgb(color)).child(s)
        };
        let mut out = Self::subtitle(t, "Web page").debug_selector(|| "web-page-heading".into()).child(self.toggle(
            t,
            "serve_web",
            "Open my sessions in a browser on my other devices",
            cx,
        ));
        match self.web() {
            WebState::Off | WebState::Waiting => {}
            WebState::NeedsTailscale => {
                out = out.child(note("It needs Tailscale access, above.".into(), t.muted, "web-needs-tailscale"));
                if !Self::is_locked(&self.row("serve_tailscale")) {
                    let on = |sw: &mut Self, _: &mut Window, cx: &mut Context<Self>| {
                        sw.choose("serve_tailscale", json!(true), cx)
                    };
                    let button = text_button(t, "web-turn-on-tailscale".into(), "Turn on Tailscale access too", cx, on);
                    out = out.child(div().flex().child(button));
                }
            }
            WebState::NotServing(why) => {
                out = out.child(note(format!("Not serving: {why}"), t.error, "web-not-serving"))
            }
            WebState::Serving { urls, error } => {
                if let Some(url) = urls.first().cloned() {
                    out = out.child(self.address(t, &url, &urls[1..], cx));
                }
                out = out.children(error.map(|e| note(e, t.error, "web-error")));
            }
        }
        let https = self.open.https.is_some();
        out.child(Self::disclosure(t, "web-https", "HTTPS (optional)", https, cx, |sw| {
            sw.open.https = match sw.open.https {
                Some(_) => None,
                None => Some(leon_hosts::tailscale::cert_tool()),
            }
        }))
        .when(https, |d| {
            d.child(note(
                "Tailscale makes the two files once (HTTPS has to be on for your tailnet):".into(),
                t.muted,
                "web-cert-note",
            ))
            .child(
                div()
                    .debug_selector(|| "web-cert-command".into())
                    .font_family("Menlo")
                    .text_size(px(SECONDARY_TEXT))
                    .text_color(rgb(t.text))
                    .child(self.cert_hint()),
            )
            .child(self.web_file(t, "serve_web_cert", cx))
            .child(self.web_file(t, "serve_web_key", cx))
        })
    }

    /// The address with Copy, its QR code, and the other addresses under a disclosure.
    fn address(&self, t: &Theme, url: &str, others: &[String], cx: &mut Context<Self>) -> gpui::Div {
        let copy = {
            let url = url.to_string();
            text_button(t, "web-copy".into(), "Copy", cx, move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(url.clone()))
            })
        };
        let line = div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .debug_selector(|| "web-url".into())
                    .font_family("Menlo")
                    .text_color(rgb(t.title))
                    .child(url.to_string()),
            )
            .child(copy);
        let mut out = div().flex().flex_col().gap(px(8.)).child(line).children(qr_code(url, t)).child(
            div()
                .debug_selector(|| "web-qr-note".into())
                .text_size(px(SECONDARY_TEXT))
                .text_color(rgb(t.muted))
                .child(QR_NOTE),
        );
        if !others.is_empty() {
            out = out.child(Self::disclosure(t, "web-others", "Other addresses", self.open.others, cx, |sw| {
                sw.open.others = !sw.open.others
            }));
        }
        if self.open.others {
            for (i, u) in others.iter().enumerate() {
                let sel = format!("web-other-url-{i}");
                out = out.child(
                    div().debug_selector(|| sel).font_family("Menlo").text_size(px(SECONDARY_TEXT)).child(u.clone()),
                );
            }
        }
        out
    }
}

/// `url`'s QR code, painted (R7): the field, then each run of dark modules, on whole points.
fn qr_code(url: &str, t: &Theme) -> Option<gpui::Div> {
    let m = qr::modules(url)?;
    let (field, module) = qr::colours(t.bg, t.title);
    let (side, runs) = (m.len() + 2 * qr::QUIET, qr::runs(&m));
    let paint = move |b: Bounds<gpui::Pixels>, _: (), window: &mut Window, _: &mut App| {
        let unit = (QR_SIDE / side as f32).floor().max(1.);
        let at = |v: usize| (QR_SIDE - unit * side as f32) / 2. + unit * (v + qr::QUIET) as f32;
        window.paint_quad(fill(b, rgb(field)));
        for &(y, x, len) in &runs {
            let origin = point(b.origin.x + px(at(x)), b.origin.y + px(at(y)));
            window.paint_quad(fill(Bounds::new(origin, size(px(unit * len as f32), px(unit))), rgb(module)));
        }
    };
    Some(
        div()
            .debug_selector(|| "web-qr".into())
            .flex_none()
            .size(px(QR_SIDE))
            .child(canvas(|_, _, _| {}, paint).size_full()),
    )
}
