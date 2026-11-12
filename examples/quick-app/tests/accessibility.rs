//! The example reaches screen readers and the keyboard with nothing written for them.

use ravenrs_app_a11y::Audit;
use ravenrs_core::Size;
use ravenrs_style::Theme;
use ravenrs_widgets::Ui;

#[test]
fn every_screen_and_its_new_record_pass_the_audit() {
    let mut ui = Ui::new(quick_app::app(), Theme::raven_dark());
    ui.set_viewport(Size::new(1200.0, 800.0), 1.0);
    let report = Audit::new().theme(Theme::raven_light()).open("Novo").app(&mut ui);
    assert!(report.places.len() >= 4, "{:?}", report.places);
    report.assert_clean();
}
