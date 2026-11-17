//! A block's title, pwd and foreground are stored with the event that announces them, under the
//! state lock (`State::notice`): a snapshot taken under that lock shows a value exactly when its
//! event has been sent, so an attach in between can't get a change twice.

use std::sync::Arc;

use leon_core::terminal::Notice;
use leon_server::{Ctx, Info};
use serde_json::json;

#[tokio::test]
async fn a_notice_is_stored_as_its_event_is_sent() {
    let c = Arc::new(Ctx::new(Info { pid: 1, version: "t", socket: "/nonexistent".into() }, false));
    let layout = json!({ "block": { "options": { "command": ["sleep", "30"] } } });
    let r = c.call("session.create", json!({ "layout": layout })).await.unwrap();
    let (sid, block) = (r["session_id"].clone(), r["block_id"].as_str().unwrap().to_string());
    {
        let mut st = c.state();
        st.notice(&block, Notice::Foreground(Some("probe".into())));
        st.notice(&block, Notice::Title("a title".into()));
    }
    let b = c.call("session.describe_block", json!({ "session_id": sid, "block_id": block })).await.unwrap();
    assert_eq!((b["foreground"].as_str(), b["title"].as_str()), (Some("probe"), Some("a title")), "{b}");
}
