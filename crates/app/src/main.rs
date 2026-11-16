//! The logic analyzer application.
//!
//! A thin iced shell: it draws what the host API describes and turns input into host commands.
//! No domain logic lives here (see `specs/006-ui-shell-signal-view`).

mod app;
mod decoder_view;
mod dialogs;
mod fonts;
mod frame_log;
mod signal_view;
mod theme;

use iced::{Point, Size, window};
use la_diagnostics::{LogConfig, init_logging, install_panic_hook};
use la_host::ShellConfig;
use la_host::settings::{RealSettingsFs, Settings};

use crate::app::App;

fn main() -> iced::Result {
    let _log = init_logging(LogConfig::default());
    install_panic_hook();
    // Where the window was last time (the settings are read again by the shell afterwards).
    let saved = Settings::load(&RealSettingsFs, &ShellConfig::for_user().settings_path)
        .settings
        .window;
    let minimum = Size::new(800.0, 560.0);
    let size = saved.map_or(Size::new(1180.0, 720.0), |g| {
        Size::new(
            (g.width as f32).max(minimum.width),
            (g.height as f32).max(minimum.height),
        )
    });
    // The place is used only when it is plausibly on a screen; the system moves a window that
    // is not visible at all, and the size alone is always restored.
    let position = saved
        .filter(|g| (-50..=7_680).contains(&g.x) && (-50..=4_320).contains(&g.y))
        .map_or(window::Position::Default, |g| {
            window::Position::Specific(Point::new(g.x as f32, g.y as f32))
        });
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .scale_factor(App::scale_factor)
        .font(fonts::JAPANESE)
        .subscription(App::subscription)
        .antialiasing(true)
        .window(window::Settings {
            size,
            position,
            min_size: Some(minimum),
            exit_on_close_request: false,
            ..window::Settings::default()
        })
        .run()
}
