//! macOS: `UNUserNotificationCenter` banners with a delegate that turns a click into
//! `notify::clicked`, and the Dock tile's badge. Called on the main thread (the window's poll).

use std::ffi::{c_char, CStr, CString};
use std::sync::Once;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, NSObject};
use objc2::{define_class, msg_send, AllocAnyThread};

use super::Click;
use crate::attention::Notice;

#[link(name = "UserNotifications", kind = "framework")]
unsafe extern "C" {}

// UNAuthorizationOption and UNNotificationPresentationOption bits.
const BADGE_SOUND_ALERT: usize = 1 | 2 | 4;
const LIST_BANNER: usize = (1 << 3) | (1 << 4);
const KEYS: [&CStr; 3] = [c"host", c"session", c"block"];

fn ns_string(s: &str) -> Retained<AnyObject> {
    let c = CString::new(s.replace('\0', "")).unwrap_or_default();
    let cls = AnyClass::get(c"NSString").expect("Foundation");
    unsafe { msg_send![cls, stringWithUTF8String: c.as_ptr()] }
}

fn rust_string(s: *mut AnyObject) -> String {
    if s.is_null() {
        return String::new();
    }
    let p: *const c_char = unsafe { msg_send![s, UTF8String] };
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and the delegate has no Drop.
    #[unsafe(super(NSObject))]
    #[name = "LeonNotificationDelegate"]
    struct Delegate;

    impl Delegate {
        /// A click: queue where it came from.
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(&self, _center: *mut AnyObject, response: *mut AnyObject, done: &block2::DynBlock<dyn Fn()>) {
            let info: *mut AnyObject = unsafe {
                let n: *mut AnyObject = msg_send![response, notification];
                let r: *mut AnyObject = msg_send![n, request];
                let c: *mut AnyObject = msg_send![r, content];
                msg_send![c, userInfo]
            };
            let get = |k: &CStr| -> String {
                let key = ns_string(k.to_str().unwrap_or_default());
                rust_string(unsafe { msg_send![info, objectForKey: &*key] })
            };
            if !info.is_null() {
                super::clicked(Click { host_label: get(KEYS[0]), session_id: get(KEYS[1]), block_id: get(KEYS[2]) });
            }
            done.call(());
        }

        /// A notice while leon is in front (only for a pane not on screen): show it anyway.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(&self, _center: *mut AnyObject, _n: *mut AnyObject, done: &block2::DynBlock<dyn Fn(usize)>) {
            done.call((LIST_BANNER,));
        }
    }
);

/// The notification center, set up once: None when this process isn't a bundled app (Apple's
/// API raises there).
fn center() -> Option<Retained<AnyObject>> {
    static SETUP: Once = Once::new();
    let bundled = unsafe {
        let bundle: *mut AnyObject = msg_send![AnyClass::get(c"NSBundle")?, mainBundle];
        let id: *mut AnyObject = msg_send![bundle, bundleIdentifier];
        !id.is_null()
    };
    if !bundled {
        SETUP.call_once(|| eprintln!("leon: not a bundled app (Leon.app), so no notifications"));
        return None;
    }
    let center: Retained<AnyObject> =
        unsafe { msg_send![AnyClass::get(c"UNUserNotificationCenter")?, currentNotificationCenter] };
    SETUP.call_once(|| unsafe {
        let delegate: Retained<Delegate> = msg_send![super(Delegate::alloc().set_ivars(())), init];
        let _: () = msg_send![&*center, setDelegate: &*delegate];
        std::mem::forget(delegate); // the center holds its delegate weakly; this one lives as long as the app
        let asked = RcBlock::new(|_granted: Bool, _error: *mut AnyObject| {});
        let _: () = msg_send![&*center, requestAuthorizationWithOptions: BADGE_SOUND_ALERT, completionHandler: &*asked];
    });
    Some(center)
}

pub(super) fn post(n: &Notice) {
    let Some(center) = center() else { return };
    unsafe {
        let content: Retained<AnyObject> =
            msg_send![AnyClass::get(c"UNMutableNotificationContent").expect("UserNotifications"), new];
        let _: () = msg_send![&*content, setTitle: &*ns_string(&n.title)];
        let _: () = msg_send![&*content, setBody: &*ns_string(&n.body)];
        let values = [ns_string(&n.host_label), ns_string(&n.session_id), ns_string(&n.block_id)];
        let keys = KEYS.map(|k| ns_string(k.to_str().unwrap_or_default()));
        let (vs, ks): (Vec<&AnyObject>, Vec<&AnyObject>) =
            (values.iter().map(|v| &**v).collect(), keys.iter().map(|k| &**k).collect());
        let dict: Retained<AnyObject> = msg_send![
            AnyClass::get(c"NSDictionary").expect("Foundation"),
            dictionaryWithObjects: vs.as_ptr(), forKeys: ks.as_ptr(), count: vs.len()
        ];
        let _: () = msg_send![&*content, setUserInfo: &*dict];
        // one per block: a newer notice for it replaces the one still showing
        let id = ns_string(&format!("leon-{}-{}", n.host_label, n.block_id));
        let request: Retained<AnyObject> = msg_send![
            AnyClass::get(c"UNNotificationRequest").expect("UserNotifications"),
            requestWithIdentifier: &*id, content: &*content, trigger: std::ptr::null::<AnyObject>()
        ];
        let _: () = msg_send![&*center, addNotificationRequest: &*request, withCompletionHandler: std::ptr::null::<AnyObject>()];
    }
}

pub(super) fn badge(text: Option<&str>) {
    unsafe {
        let Some(cls) = AnyClass::get(c"NSApplication") else { return };
        let app: *mut AnyObject = msg_send![cls, sharedApplication];
        let tile: *mut AnyObject = msg_send![app, dockTile];
        match text {
            Some(t) => {
                let _: () = msg_send![tile, setBadgeLabel: &*ns_string(t)];
            }
            None => {
                let _: () = msg_send![tile, setBadgeLabel: std::ptr::null::<AnyObject>()];
            }
        }
    }
}
