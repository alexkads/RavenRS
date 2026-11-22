//! The actions a component declares with `BuildCx::on_access`, asked for as assistive
//! technology asks, and a node busy with work that has not finished (#735).

use std::cell::RefCell;
use std::rc::Rc;

use ravenrs_core::{AccessAction, Role};
use ravenrs_e2e::{E2e, ErrorCode};
use ravenrs_layout::{Axis, LayoutStyle};
use ravenrs_reactive::State;
use ravenrs_style::Theme;
use ravenrs_widgets::{BuildCx, BusySignal, NodeId, Paint, Semantics, VStack, View, ViewExt};

fn block(cx: &mut BuildCx<'_>) -> NodeId {
    let style =
        LayoutStyle { width: Some(120.0), height: Some(32.0), ..LayoutStyle::stack(Axis::Horizontal) };
    cx.container(style, &[], Paint::default())
}

/// Each action a node was asked for, with the text that came with it.
type Log = Rc<RefCell<Vec<(AccessAction, Option<String>)>>>;

/// A node that declares `actions` and writes down each one it is asked for, with its text.
struct Takes {
    actions: Vec<AccessAction>,
    log: Log,
    expanded: Option<State<bool>>,
}

impl View for Takes {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = block(cx);
        let mut semantics = Semantics::new(Role::TreeItem).name("Item");
        if let Some(open) = &self.expanded {
            semantics = semantics.expanded(open.get());
        }
        cx.semantics(node, semantics);
        let log = self.log.clone();
        let open = self.expanded.clone();
        cx.on_access(node, &self.actions, move |action, text| {
            log.borrow_mut().push((action, text.map(str::to_owned)));
            if let Some(open) = &open {
                match action {
                    AccessAction::Expand => open.set(true),
                    AccessAction::Collapse => open.set(false),
                    _ => {}
                }
            }
            true
        });
        node
    }
}

fn taking(actions: &[AccessAction]) -> (E2e, Log) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let view = Takes { actions: actions.to_vec(), log: log.clone(), expanded: None }.test_id("item");
    (E2e::new(VStack::new((view,)), Theme::raven_dark()), log)
}

#[test]
fn each_new_action_reaches_the_component_that_declares_it() {
    use AccessAction::*;
    for action in [Expand, Collapse, Select, ScrollIntoView, ShowMenu, Dismiss] {
        let (mut e2e, log) = taking(&[action]);
        e2e.by_test_id("item").perform(action).unwrap();
        assert_eq!(log.borrow().as_slice(), &[(action, None)], "{action:?}");
    }
    let (mut e2e, log) = taking(&[SetValue]);
    e2e.by_test_id("item").set_value("10,50").unwrap();
    assert_eq!(log.borrow().as_slice(), &[(SetValue, Some("10,50".to_owned()))]);
    let (mut e2e, log) = taking(&[ShowMenu, Dismiss]);
    e2e.by_test_id("item").show_menu().unwrap();
    e2e.by_test_id("item").dismiss().unwrap();
    assert_eq!(log.borrow().len(), 2);
}

#[test]
fn an_action_the_node_does_not_declare_is_refused_by_name() {
    let (mut e2e, log) = taking(&[AccessAction::Expand]);
    let error = e2e.by_test_id("item").show_menu().unwrap_err();
    assert_eq!(error.code(), ErrorCode::Disabled);
    assert!(error.to_string().contains("ShowMenu"), "{error}");
    assert!(log.borrow().is_empty());
}

#[test]
fn expand_uses_the_declared_action_instead_of_a_click() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let open = State::new(false);
    let view = Takes {
        actions: vec![AccessAction::Expand, AccessAction::Collapse],
        log: log.clone(),
        expanded: Some(open.clone()),
    }
    .test_id("item");
    let mut e2e = E2e::new(VStack::new((view,)), Theme::raven_dark());
    e2e.by_test_id("item").expand().unwrap();
    assert!(open.get());
    e2e.by_test_id("item").collapse().unwrap();
    assert!(!open.get());
    assert_eq!(
        log.borrow().iter().map(|(a, _)| *a).collect::<Vec<_>>(),
        [AccessAction::Expand, AccessAction::Collapse]
    );
}

/// A button busy until the session's clock reaches `until`.
struct BusyUntil {
    until: f64,
    clicks: State<u32>,
    signal: Option<BusySignal>,
}

impl View for BusyUntil {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let node = block(cx);
        cx.semantics(node, Semantics::new(Role::Button).name("Send"));
        let clicks = self.clicks.clone();
        cx.on_click(node, Rc::new(move || clicks.update(|n| *n += 1)));
        match &self.signal {
            Some(signal) => cx.busy_while(node, signal),
            None => {
                let busy = cx.now() < self.until;
                cx.busy(node, busy);
                if busy {
                    cx.wake_at(self.until);
                }
            }
        }
        node
    }
}

#[test]
fn a_click_on_a_busy_node_waits_on_the_session_clock_until_it_is_free() {
    let clicks = State::new(0);
    let view = BusyUntil { until: 1.5, clicks: clicks.clone(), signal: None }.test_id("send");
    let mut e2e = E2e::new(VStack::new((view,)), Theme::raven_dark());
    assert!(e2e.snapshot().contains("busy"), "{}", e2e.snapshot());
    let real = std::time::Instant::now();
    e2e.by_test_id("send").click().unwrap();
    assert_eq!(clicks.get(), 1);
    assert!(e2e.clock() >= 1.5);
    assert!(real.elapsed() < std::time::Duration::from_secs(1), "it waited on the session's clock");
    assert!(!e2e.snapshot().contains("busy"));
}

#[test]
fn a_node_that_stays_busy_fails_saying_so() {
    let signal = BusySignal::new();
    signal.set(true);
    let clicks = State::new(0);
    let view = BusyUntil { until: 0.0, clicks: clicks.clone(), signal: Some(signal) }.test_id("send");
    let mut e2e = E2e::new(VStack::new((view,)), Theme::raven_dark());
    let error = e2e.by_test_id("send").click().unwrap_err();
    assert_eq!(error.code(), ErrorCode::Timeout);
    assert!(error.to_string().contains("busy"), "{error}");
    assert_eq!(clicks.get(), 0);
}

#[test]
fn a_chat_is_busy_while_it_waits_for_the_answer() {
    use ravenrs_widgets::{Chat, ChatMessage};
    let messages = State::new(vec![ChatMessage::assistant("Olá")]);
    let waiting = State::new(true);
    let (m, w) = (messages.clone(), waiting.clone());
    let view = ravenrs_widgets::view_fn(move || Chat::new(m.binding()).busy(w.get()).test_id("chat"));
    let mut e2e = E2e::new(view, Theme::raven_dark());
    assert!(e2e.by_test_id("chat").node().unwrap().busy, "{}", e2e.snapshot());
    waiting.set(false);
    e2e.advance(0.1);
    assert!(!e2e.by_test_id("chat").node().unwrap().busy, "{}", e2e.snapshot());
}
