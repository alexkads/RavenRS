//! The session's clock, and steps that wait for the screen.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use ravenrs_e2e::{E2e, ErrorCode};
use ravenrs_style::Theme;
use ravenrs_widgets::{BuildCx, NodeId, Text, VStack, View, ViewExt};

/// Says "Pronto" half a second after the screen opens, by a timer of the screen itself.
struct Later;

impl View for Later {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let ready = cx.now() >= 0.5;
        if !ready {
            cx.wake_at(0.5);
        }
        cx.child(&Text::new(if ready { "Pronto" } else { "Carregando" }).test_id("aviso"))
    }
}

/// Says "Recebido" once work done on another thread wakes the screen.
struct Background(Arc<AtomicBool>);

impl View for Background {
    fn build(&self, cx: &mut BuildCx<'_>) -> NodeId {
        let done = self.0.load(Ordering::SeqCst);
        cx.child(&Text::new(if done { "Recebido" } else { "Esperando" }).test_id("resposta"))
    }
}

fn session(view: impl View + 'static) -> E2e {
    E2e::new(VStack::new((view,)), Theme::raven_dark())
}

#[test]
fn the_clock_moves_only_when_the_session_moves_it() {
    let mut e2e = session(Later);
    assert_eq!(e2e.clock(), 0.0);
    e2e.by_test_id("aviso").expect().to_have_name("Carregando").unwrap();
    e2e.advance(0.25);
    e2e.by_test_id("aviso").expect().to_have_name("Carregando").unwrap();
    e2e.advance(0.3);
    e2e.by_test_id("aviso").expect().to_have_name("Pronto").unwrap();
    assert!((e2e.clock() - 0.55).abs() < 1e-9, "{}", e2e.clock());
}

#[test]
fn a_wait_sees_a_timer_fire_and_times_out_with_the_screen_when_it_does_not() {
    let mut e2e = session(Later);
    let short = e2e.by_test_id("aviso").wait_to_mention("Pronto", Duration::from_millis(100)).unwrap_err();
    assert_eq!(short.code(), ErrorCode::Timeout);
    assert!(short.message().starts_with("waited"), "{short}");
    assert!(short.screen_text().contains("Carregando"), "{}", short.screen_text());
    e2e.by_test_id("aviso").wait_to_mention("Pronto", Duration::from_secs(3)).unwrap();
}

#[test]
fn a_wait_sees_work_done_in_the_background() {
    let done = Arc::new(AtomicBool::new(false));
    let mut e2e = session(Background(done.clone()));
    let waker = e2e.ui_mut().ui_waker();
    let worker = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(80));
        done.store(true, Ordering::SeqCst);
        waker.wake();
    });
    e2e.by_test_id("resposta").wait_to_mention("Recebido", Duration::from_secs(3)).unwrap();
    worker.join().unwrap();
}

#[test]
fn a_patient_step_passes_once_ready_and_fails_with_its_own_code_when_never() {
    let mut e2e = session(Later);
    e2e.patiently(Duration::from_secs(3), |e| {
        e.by_text("Pronto").click().or_else(|_| e.by_test_id("aviso").expect().to_have_name("Pronto"))
    })
    .unwrap();
    let started = Instant::now();
    let never = e2e.patiently(Duration::from_millis(150), |e| e.by_test_id("nunca").click()).unwrap_err();
    assert_eq!(never.code(), ErrorCode::NotFound, "the step's own code, not a timeout");
    assert!(started.elapsed() >= Duration::from_millis(150));
}
