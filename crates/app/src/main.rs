//! The logic analyzer application.
//!
//! A thin iced shell: it draws what the host API describes and turns input into host commands.
//! No domain logic lives here (see `specs/006-ui-shell-signal-view`).

use la_diagnostics::{LogConfig, init_logging, install_panic_hook};
use la_i18n::{Key, Localizer};

/// The application state of the first window (grows with the host shell in later tasks).
struct App {
    localizer: Localizer,
}

impl App {
    fn new() -> Self {
        Self {
            localizer: Localizer::from_settings(None, &[]),
        }
    }

    fn title(&self) -> String {
        self.localizer
            .message(&Key::from_static("app-title"), &la_i18n::Args::new())
    }

    fn update(&mut self, _message: ()) {}

    fn view(&self) -> iced::Element<'_, ()> {
        iced::widget::space().into()
    }
}

fn main() -> iced::Result {
    let _log = init_logging(LogConfig::default());
    install_panic_hook();
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .run()
}
