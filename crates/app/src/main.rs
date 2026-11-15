//! The logic analyzer application.
//!
//! A thin iced shell: it draws what the host API describes and turns input into host commands.
//! No domain logic lives here (see `specs/006-ui-shell-signal-view`).

mod app;
mod dialogs;
mod frame_log;
mod signal_view;
mod theme;

use iced::{Size, window};
use la_diagnostics::{LogConfig, init_logging, install_panic_hook};

use crate::app::App;

fn main() -> iced::Result {
    let _log = init_logging(LogConfig::default());
    install_panic_hook();
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .subscription(App::subscription)
        .antialiasing(true)
        .window(window::Settings {
            size: Size::new(1180.0, 720.0),
            min_size: Some(Size::new(800.0, 560.0)),
            exit_on_close_request: false,
            ..window::Settings::default()
        })
        .run()
}
