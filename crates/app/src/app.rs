//! The application: state, messages, update and view.
//!
//! Everything the application does is a command of the host; this module maps input to commands
//! and the host state to widgets. It contains no domain logic.

use std::fmt;
use std::time::{Duration, Instant};

use iced::widget::{
    Space, button, canvas, checkbox, column, container, mouse_area, pick_list, progress_bar, row,
    scrollable, stack, text, text_input, tooltip,
};
use iced::{Element, Fill, Length, Subscription, Task, Theme, keyboard, theme, window};
use la_decoders::setup::{Source, Value};
use la_devices::DeviceId;
use la_host::decoders::{CatalogueOptionKind, InstanceState};
use la_host::presentation::{Colour, PALETTE_SIZE};
use la_host::shortcuts::{Accelerator, Action, ShortcutEntry};
use la_host::window::ViewSize;
use la_host::{
    Activity, Changes, Command, ConfigEdit, Decision, Outcome, Shell, ShellConfig, SignalProfile,
    Tick, Unsaved,
};
use la_i18n::{Args, Key};

use crate::signal_view::SignalView;
use crate::theme::Variant;

/// Everything that can happen to the application.
#[derive(Debug, Clone)]
pub enum Message {
    /// The background-work timer fired.
    Tick(Instant),
    /// A key event.
    Key(keyboard::Event),
    /// The signal area has this width in logical pixels.
    AreaResized(f32),
    /// An icon of the strip at the right edge was clicked: this is the view that results.
    SidePanel(la_host::SidePanel),
    /// The user clicked the name of a channel (its number and the top of its lane in the view):
    /// the in-place editor opens over it.
    EditChannelName(u8, f32),
    /// A lane was dragged from the lane `from` to the lane `to` of the signal view (positions in
    /// the list of the lanes that are shown).
    MoveLane {
        /// Where the lane was.
        from: usize,
        /// Where it was dropped.
        to: usize,
    },
    /// The text of the in-place editor changed.
    ChannelNameInput(String),
    /// The editor is accepted (Enter, or a click elsewhere with a changed text).
    ChannelNameAccept,
    /// The editor is cancelled (Escape).
    ChannelNameCancel,
    /// Run a host command.
    Run(Command),
    /// A menu title was clicked.
    MenuToggled(usize),
    /// The open menu is dismissed.
    MenuClosed,
    /// A device was chosen.
    DevicePicked(DeviceChoice),
    /// A number of channels was chosen.
    /// The sample rate text changed.
    RateText(String),
    /// The capture length text changed.
    LengthText(String),
    /// The seed text changed.
    SeedText(String),
    /// A signal profile was chosen.
    ProfilePicked(ProfileChoice),
    /// The real-time checkbox changed.
    RealtimeToggled(bool),
    /// The system theme changed.
    SystemTheme(theme::Mode),
    /// The window asked to close.
    CloseRequested,
    /// A channel was renamed in the channel panel.
    ChannelName(u8, String),
    /// A channel was shown or hidden.
    ChannelVisible(u8, bool),
    /// The colour of a channel moves to the next palette entry.
    ChannelColour(u8),
    /// A channel moves one position up (negative) or down (positive) in the order.
    ChannelMove(usize, i8),
    /// Follow the newest data on or off.
    ToggleFollow,
    /// Open the folder with the log files.
    ShowLog,
    /// Show or hide the list of recent messages.
    ToggleMessages,
    /// Open or close the settings dialog.
    OpenSettings,
    /// Close the settings dialog.
    CloseSettings,
    /// Open the about dialog.
    ShowAbout,
    /// Close the about dialog.
    CloseAbout,
    /// A language was chosen in the settings.
    LanguagePicked(Choice<la_i18n::Language>),
    /// A theme was chosen in the settings.
    ThemePicked(Choice<la_host::settings::Theme>),
    /// The interface scale goes to the next larger (+1) or smaller (-1) step.
    ScaleStep(i8),
    /// The text of the memory limit field changed.
    MemoryText(String),
    /// The text of the decoder memory limit field changed.
    DecoderMemoryText(String),
    /// The window moved or changed size.
    WindowEvent(window::Event),
    /// Ask for a file and open it.
    OpenFile,
    /// Save to the current file, or ask for a name when there is none.
    SaveFile,
    /// Ask for a name and save to it.
    SaveFileAs,
    /// The file chooser ended (`None`: the user gave up).
    FileChosen(FileIntent, Option<std::path::PathBuf>),
    /// A marker shortcut or menu entry: place, previous, next, remove selected, snap.
    MarkerAction(MarkerAction),
    /// A marker was renamed in the markers panel.
    MarkerName(u32, String),
    /// A marker becomes the first (0) or second (1) of the measurement.
    MarkerMeasure(usize, u32),
    /// The menu entry that adds a decoder.
    AddDecoderMenu,
    /// A source was chosen for a decoder instance (the capture, or another instance).
    DecoderSourcePicked(u32, Choice<Source>),
    /// A decoder was chosen in the list of decoders to add.
    AddDecoderPicked(Choice<String>),
    /// A decoder instance was renamed in the panel.
    DecoderName(u32, String),
    /// A channel was chosen for a signal of a decoder instance.
    DecoderChannelPicked(u32, String, Choice<Option<u8>>),
    /// A choice option of a decoder instance was changed.
    DecoderOptionPicked(u32, String, Choice<String>),
    /// The text of a number option of a decoder instance changed.
    DecoderOptionText(u32, String, String),
    /// A switch option of a decoder instance was changed.
    DecoderOptionFlag(u32, String, bool),
    /// A generator option of a decoder instance changed (`errors` or `error_rate`).
    DecoderGenerator(u32, &'static str, i64),
    /// The text of the generator's error rate changed.
    DecoderRateText(u32, String),
    /// Nothing to do.
    Noop,
}

/// A value in a chooser with the text to show for it (the text is computed when the view is
/// built, so it is in the language in use).
#[derive(Debug, Clone)]
pub struct Choice<T> {
    value: T,
    label: String,
}

impl<T: PartialEq> PartialEq for Choice<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The name of a language in that language (a proper name: it does not follow the language in
/// use).
fn autonym(localizer: &la_i18n::Localizer, language: la_i18n::Language) -> String {
    use la_i18n::Language;
    let key = match language {
        Language::En => Key::from_static("language-name-en"),
        Language::De => Key::from_static("language-name-de"),
        Language::Fr => Key::from_static("language-name-fr"),
        Language::Es => Key::from_static("language-name-es"),
        Language::Ja => Key::from_static("language-name-ja"),
        Language::Uk => Key::from_static("language-name-uk"),
    };
    localizer.message_in(language, &key, &Args::new())
}

/// The interface scales the shortcuts and the settings step through, in percent.
const SCALE_STEPS: [u16; 6] = [75, 100, 125, 150, 175, 200];
/// The smallest and the largest step.
const SCALE_MIN: u16 = 75;
const SCALE_MAX: u16 = 200;

/// What a file chooser was opened for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIntent {
    /// Choose a file to open.
    Open,
    /// Choose a name to save to.
    SaveAs,
}

/// What a marker menu entry or shortcut asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerAction {
    /// Place a marker at the centre of the view.
    Place,
    /// Go to the previous marker.
    Previous,
    /// Go to the next marker.
    Next,
    /// Remove the selected marker.
    RemoveSelected,
    /// Turn snapping on or off.
    ToggleSnap,
}

/// A device in the device chooser.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceChoice {
    id: DeviceId,
    label: String,
}

impl fmt::Display for DeviceChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// A signal profile in the chooser, with its localized name.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileChoice {
    profile: SignalProfile,
    label: String,
}

impl fmt::Display for ProfileChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The key of an accelerator as the host table names it.
fn key_name(key: &keyboard::Key) -> Option<String> {
    use keyboard::key::Named;
    match key {
        keyboard::Key::Named(named) => {
            let name = match named {
                Named::F1 => "F1",
                Named::F2 => "F2",
                Named::F3 => "F3",
                Named::F4 => "F4",
                Named::F5 => "F5",
                Named::F6 => "F6",
                Named::F7 => "F7",
                Named::F8 => "F8",
                Named::F9 => "F9",
                Named::F10 => "F10",
                Named::F11 => "F11",
                Named::F12 => "F12",
                Named::ArrowLeft => "Left",
                Named::ArrowRight => "Right",
                Named::Home => "Home",
                Named::End => "End",
                Named::Delete => "Delete",
                Named::Escape => "Escape",
                _ => return None,
            };
            Some(name.to_owned())
        }
        keyboard::Key::Character(text) => Some(text.to_uppercase()),
        keyboard::Key::Unidentified => None,
    }
}

/// The accelerator a key press stands for.
#[must_use]
pub fn accelerator_of(
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<(Accelerator, String)> {
    let name = key_name(key)?;
    let accelerator = Accelerator {
        ctrl: modifiers.control(),
        shift: modifiers.shift(),
        alt: modifiers.alt(),
        key: "",
    };
    Some((accelerator, name))
}

/// The table entry for a key press, if there is one.
#[must_use]
pub fn entry_for<'a>(
    table: &'a [ShortcutEntry],
    key: &keyboard::Key,
    modifiers: keyboard::Modifiers,
) -> Option<&'a ShortcutEntry> {
    let (pressed, name) = accelerator_of(key, modifiers)?;
    let exact = table.iter().find(|entry| {
        entry.accelerator.is_some_and(|a| {
            a.key == name
                && a.ctrl == pressed.ctrl
                && a.shift == pressed.shift
                && a.alt == pressed.alt
        })
    });
    // The plus key is a shifted `=` on many layouts: either one stands for the other, with
    // Shift not counting.
    exact.or_else(|| {
        let alias = match name.as_str() {
            "=" => "+",
            "+" => "=",
            _ => return None,
        };
        table.iter().find(|entry| {
            entry
                .accelerator
                .is_some_and(|a| a.key == alias && a.ctrl == pressed.ctrl && a.alt == pressed.alt)
        })
    })
}

/// The application.
pub struct App {
    shell: Shell,
    table: Vec<ShortcutEntry>,
    variant: Variant,
    system_mode: theme::Mode,
    plan: la_host::plan::FramePlan,
    plan_key: (u64, u64, u64, u64, u64, u32, bool),
    cache: canvas::Cache,
    cache_key: (u64, Variant, la_i18n::Language),
    open_menu: Option<usize>,
    rate_text: String,
    length_text: String,
    seed_text: String,
    names: std::collections::BTreeMap<u8, String>,
    marker_names: std::collections::BTreeMap<u32, String>,
    decoder_names: std::collections::BTreeMap<u32, String>,
    option_texts: std::collections::BTreeMap<(u32, String), String>,
    messages_open: bool,
    settings_open: bool,
    about_open: bool,
    memory_text: String,
    decoder_memory_text: String,
    window_rect: [i32; 4],
    editing: Option<ChannelEdit>,
}

/// The in-place editor of a channel name.
#[derive(Debug, Clone)]
struct ChannelEdit {
    channel: u8,
    /// The top of the lane in the signal view, in logical pixels.
    top: f32,
    text: String,
    original: String,
}

/// The identity of the in-place editor, for focusing it.
const NAME_EDITOR: &str = "channel-name-editor";

impl App {
    /// Builds the application with the host shell of the current user.
    #[must_use]
    pub fn new() -> Self {
        Self::with_shell(Shell::new(ShellConfig::for_user()))
    }

    /// Builds the application around an existing host shell.
    #[must_use]
    pub fn with_shell(shell: Shell) -> Self {
        let table = shell.shortcut_table();
        let variant = Variant::resolve(shell.state().settings.theme, theme::Mode::Dark);
        let mut app = App {
            plan: shell.frame_plan(ViewSize::default()),
            shell,
            table,
            variant,
            system_mode: theme::Mode::Dark,
            plan_key: (0, 0, 0, 0, 0, 0, false),
            cache: canvas::Cache::new(),
            cache_key: (0, variant, la_i18n::Language::En),
            open_menu: None,
            rate_text: String::new(),
            length_text: String::new(),
            seed_text: String::new(),
            names: std::collections::BTreeMap::new(),
            marker_names: std::collections::BTreeMap::new(),
            decoder_names: std::collections::BTreeMap::new(),
            option_texts: std::collections::BTreeMap::new(),
            messages_open: false,
            settings_open: false,
            about_open: false,
            memory_text: String::new(),
            decoder_memory_text: String::new(),
            window_rect: [0, 0, 0, 0],
            editing: None,
        };
        app.sync_texts();
        app.sync_names();
        app.refresh();
        app
    }

    /// The positions in the order of all channels (`MoveChannel`) for a lane dragged from the lane
    /// `from` to the lane `to` of the lanes that are shown: the moved channel goes where the channel
    /// of the target lane is.
    fn presentation_move(&self, from: usize, to: usize) -> Option<(usize, usize)> {
        let order: Vec<u8> = self
            .shell
            .state()
            .presentation
            .channels
            .iter()
            .map(|c| c.index)
            .collect();
        let position = |lane: usize| {
            let channel = self.plan.channels.get(lane)?.channel;
            order.iter().position(|c| *c == channel)
        };
        let (from, to) = (position(from)?, position(to)?);
        (from != to).then_some((from, to))
    }

    fn t(&self, key: &'static str) -> String {
        self.shell.text(&Key::from_static(key), &Args::new())
    }

    /// Copies the numbers of the configuration into the text fields.
    fn sync_texts(&mut self) {
        let config = &self.shell.state().config;
        self.rate_text = config.sample_rate_hz.to_string();
        self.length_text = config.length.to_string();
        self.seed_text = config.seed.map_or_else(String::new, |s| s.to_string());
    }

    /// Whether `mib` is a memory limit the host accepts.
    fn memory_limit_ok(&self, mib: u64) -> bool {
        mib.checked_mul(1 << 20)
            .is_some_and(la_host::settings::memory_limit_is_valid)
    }

    /// Whether `mib` is a decoder memory limit the host accepts.
    fn decoder_memory_ok(mib: u64) -> bool {
        mib.checked_mul(1 << 20)
            .is_some_and(la_host::settings::decoder_memory_limit_is_valid)
    }

    /// The settings dialog: language, theme, interface scale and memory limit. Every choice
    /// applies at once, so the dialog itself changes language when the language does.
    fn settings_dialog(&self) -> Element<'_, Message> {
        let settings = &self.shell.state().settings;
        let languages: Vec<Choice<la_i18n::Language>> = la_i18n::Language::ALL
            .iter()
            .map(|l| Choice {
                value: *l,
                label: autonym(self.shell.localizer(), *l),
            })
            .collect();
        let current_language = languages
            .iter()
            .find(|c| c.value == self.shell.state().language)
            .cloned();
        let themes: Vec<Choice<la_host::settings::Theme>> = [
            (la_host::settings::Theme::System, "theme-system"),
            (la_host::settings::Theme::Light, "theme-light"),
            (la_host::settings::Theme::Dark, "theme-dark"),
            (
                la_host::settings::Theme::HighContrast,
                "theme-high-contrast",
            ),
        ]
        .into_iter()
        .map(|(value, key)| Choice {
            value,
            label: self.t(key),
        })
        .collect();
        let current_theme = themes.iter().find(|c| c.value == settings.theme).cloned();
        let scale = settings.ui_scale_percent;
        let (low, high) = la_host::settings::memory_limit_bounds();
        let (low_mib, high_mib) = (low >> 20, high >> 20);
        let memory_valid = self
            .memory_text
            .trim()
            .parse::<u64>()
            .is_ok_and(|mib| self.memory_limit_ok(mib));
        let mut body = column![
            text(self.t("settings-title")).size(18),
            labelled(
                self.t("settings-language"),
                pick_list(languages, current_language, Message::LanguagePicked).into()
            ),
            labelled(
                self.t("settings-theme"),
                pick_list(themes, current_theme, Message::ThemePicked).into()
            ),
            labelled(
                self.t("settings-scale"),
                row![
                    button(text("\u{2212}").size(14))
                        .on_press_maybe((scale > SCALE_MIN).then_some(Message::ScaleStep(-1))),
                    text(format!("{scale} %"))
                        .width(60)
                        .align_x(iced::alignment::Horizontal::Center),
                    button(text("+").size(14))
                        .on_press_maybe((scale < SCALE_MAX).then_some(Message::ScaleStep(1))),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
            ),
            labelled(
                self.t("settings-memory"),
                row![
                    text_input("", &self.memory_text)
                        .on_input(Message::MemoryText)
                        .width(110),
                    text(self.t("settings-memory-unit")),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
            ),
            text(self.t("settings-memory-note")).size(12),
            labelled(
                self.t("settings-decoder-memory"),
                row![
                    text_input("", &self.decoder_memory_text)
                        .on_input(Message::DecoderMemoryText)
                        .width(110),
                    text(self.t("settings-memory-unit")),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center)
                .into()
            ),
            text(self.t("settings-decoder-memory-note")).size(12),
            checkbox(self.shell.state().decoders.save_results)
                .label(self.t("settings-save-decoder-results"))
                .on_toggle(|save| Message::Run(Command::SetSaveDecoderResults(save))),
            text(self.t("settings-save-decoder-results-note")).size(12),
        ]
        .spacing(10)
        .max_width(460);
        let (decoder_low, decoder_high) = la_host::settings::DECODER_MEMORY_LIMIT_RANGE;
        let decoder_memory_valid = self
            .decoder_memory_text
            .trim()
            .parse::<u64>()
            .is_ok_and(Self::decoder_memory_ok);
        if !decoder_memory_valid {
            let message = self.shell.localizer().message(
                &Key::from_static("settings-memory-invalid"),
                &Args::new()
                    .uint("min", decoder_low >> 20)
                    .uint("max", decoder_high >> 20),
            );
            body = body.push(text(message).size(13).style(text::danger));
        }
        if !memory_valid {
            let message = self.shell.localizer().message(
                &Key::from_static("settings-memory-invalid"),
                &Args::new().uint("min", low_mib).uint("max", high_mib),
            );
            body = body.push(text(message).size(13).style(text::danger));
        }
        body = body.push(
            button(text(self.t("dialog-close")))
                .style(button::secondary)
                .on_press(Message::CloseSettings),
        );
        container(container(body).padding(20).style(container::bordered_box))
            .center(Fill)
            .into()
    }

    /// The about dialog.
    fn about_dialog(&self) -> Element<'_, Message> {
        let version = self.shell.localizer().message(
            &Key::from_static("about-body"),
            &Args::new().text("version", env!("CARGO_PKG_VERSION")),
        );
        let body = column![
            text(self.t("about-title")).size(18),
            text(version),
            text(self.t("about-license")).size(13),
            button(text(self.t("dialog-close")))
                .style(button::secondary)
                .on_press(Message::CloseAbout),
        ]
        .spacing(12)
        .max_width(460);
        container(container(body).padding(20).style(container::bordered_box))
            .center(Fill)
            .into()
    }

    /// Opens the file chooser for `intent` without blocking the window.
    fn choose_file(&self, intent: FileIntent) -> Task<Message> {
        let filter = self.t("dialog-filter-session");
        match intent {
            FileIntent::Open => {
                let title = self.t("dialog-open-title");
                Task::perform(crate::dialogs::pick_open(title, filter), move |path| {
                    Message::FileChosen(intent, path)
                })
            }
            FileIntent::SaveAs => {
                let title = self.t("dialog-save-title");
                let name = self.default_file_name();
                Task::perform(
                    crate::dialogs::pick_save(title, filter, name),
                    move |path| Message::FileChosen(intent, path),
                )
            }
        }
    }

    /// The name offered when saving: that of the current file, or one made from the capture.
    fn default_file_name(&self) -> String {
        let state = self.shell.state();
        if let Some(name) = state
            .session_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
        {
            return name;
        }
        let stem = state
            .capture
            .as_ref()
            .map(|c| c.description.device_name().to_owned())
            .map(|n| {
                n.chars()
                    .map(|ch| if ch.is_alphanumeric() { ch } else { '-' })
                    .collect::<String>()
            })
            .filter(|n| !n.trim_matches('-').is_empty())
            .unwrap_or_else(|| "capture".to_owned());
        format!("{stem}.lasession")
    }

    /// Runs a marker action chosen from a menu or a shortcut.
    fn marker_action(&mut self, action: MarkerAction) -> Task<Message> {
        let state = self.shell.state();
        let window = state.window;
        let limits_length = state.capture.as_ref().map_or(0, |c| c.snapshot.length());
        let centre = if state.auto_fit {
            limits_length / 2
        } else {
            window.start.saturating_add(window.length / 2)
        };
        match action {
            MarkerAction::Place => {
                let channel = self.plan.channels.first().map_or(0, |c| c.channel);
                self.run(Command::PlaceMarker {
                    position: centre,
                    channel,
                })
            }
            MarkerAction::Previous | MarkerAction::Next => {
                let mut by_position: Vec<(u64, u32)> = state
                    .markers
                    .list
                    .iter()
                    .map(|m| (m.position, m.id))
                    .collect();
                by_position.sort_unstable();
                let target = if action == MarkerAction::Next {
                    by_position
                        .iter()
                        .find(|(p, _)| *p > centre)
                        .or(by_position.first())
                } else {
                    by_position
                        .iter()
                        .rev()
                        .find(|(p, _)| *p < centre)
                        .or(by_position.last())
                };
                match target {
                    Some((_, id)) => self.run(Command::GoToMarker(*id)),
                    None => Task::none(),
                }
            }
            MarkerAction::RemoveSelected => {
                let id = state.markers.selected[1]
                    .or(state.markers.selected[0])
                    .or_else(|| state.markers.list.last().map(|m| m.id));
                match id {
                    Some(id) => self.run(Command::RemoveMarker(id)),
                    None => Task::none(),
                }
            }
            MarkerAction::ToggleSnap => {
                let snap = !state.snap;
                self.run(Command::SetSnap(snap))
            }
        }
    }

    /// Copies the marker names into the text fields.
    fn sync_marker_names(&mut self) {
        self.marker_names = self
            .shell
            .state()
            .markers
            .list
            .iter()
            .map(|m| (m.id, m.name.clone()))
            .collect();
    }

    /// Copies the channel names of the presentation into the text fields.
    fn sync_names(&mut self) {
        self.names = self
            .shell
            .state()
            .presentation
            .channels
            .iter()
            .map(|style| (style.index, style.name.clone()))
            .collect();
    }

    /// Recomputes the frame plan and the canvas cache when what they depend on changed.
    fn refresh(&mut self) {
        let state = self.shell.state();
        let key = (
            state.versions.capture,
            state.versions.window,
            state.versions.presentation,
            state.versions.markers,
            state.versions.decoders,
            state.view_size.pixels,
            state.auto_fit,
        );
        if key != self.plan_key {
            self.plan_key = key;
            let started = Instant::now();
            self.plan = self.shell.frame_plan(state.view_size);
            crate::frame_log::record(crate::frame_log::Stage::Plan, started.elapsed());
            self.cache.clear();
        }
        let cache_key = (
            state.versions.theme,
            self.variant,
            self.shell.localizer().language(),
        );
        if cache_key != self.cache_key {
            self.cache_key = cache_key;
            self.cache.clear();
        }
    }

    /// The window title.
    #[must_use]
    pub fn title(&self) -> String {
        let state = self.shell.state();
        if state.capture.is_none() {
            return self.t("app-title");
        }
        let name = state
            .session_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(
                || self.t("title-untitled"),
                |n| n.to_string_lossy().into_owned(),
            );
        let mark = if state.unsaved == Unsaved::Clean {
            ""
        } else {
            "*"
        };
        format!("{name}{mark} \u{2014} {}", self.t("app-title"))
    }

    /// The interface scale as a factor.
    #[must_use]
    pub fn scale_factor(&self) -> f32 {
        f32::from(self.shell.state().settings.ui_scale_percent) / 100.0
    }

    /// The theme in use.
    #[must_use]
    pub fn theme(&self) -> Theme {
        self.variant.theme()
    }

    /// The subscriptions: keyboard, window close, system theme and the background-work timer.
    pub fn subscription(&self) -> Subscription<Message> {
        let period = match self.shell.wants_ticks() {
            Tick::Fast => Duration::from_millis(50),
            Tick::Slow => Duration::from_millis(500),
        };
        Subscription::batch([
            keyboard::listen().map(Message::Key),
            iced::time::every(period).map(Message::Tick),
            window::close_requests().map(|_| Message::CloseRequested),
            // Only the moves and size changes are wanted: turning every window event (redraw
            // requests included) into a message would make the toolkit redraw for each of them.
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Window(
                    moved @ (window::Event::Moved(_) | window::Event::Resized(_)),
                ) => Some(Message::WindowEvent(moved)),
                _ => None,
            }),
            iced::system::theme_changes().map(Message::SystemTheme),
        ])
    }

    /// Runs a host command and turns its outcome into a task.
    fn run(&mut self, command: Command) -> Task<Message> {
        let renames = matches!(
            command,
            Command::RenameChannel { .. } | Command::RenameMarker { .. }
        );
        let outcome = self.shell.apply(command);
        if !renames {
            self.sync_names();
            self.sync_marker_names();
        }
        self.after(&outcome)
    }

    fn after(&mut self, outcome: &Outcome) -> Task<Message> {
        self.sync_after_command();
        match outcome {
            Outcome::Done(changes) if changes.contains(Changes::QUIT) => {
                // What is only kept in memory while running (the window place) is remembered.
                self.shell.save_settings();
                iced::exit()
            }
            // Saving was asked for but the capture has no file yet: ask for a name.
            Outcome::Ignored(reason) if *reason == la_host::NEEDS_PATH => {
                self.choose_file(FileIntent::SaveAs)
            }
            _ => Task::none(),
        }
    }

    fn sync_after_command(&mut self) {
        self.variant = Variant::resolve(self.shell.state().settings.theme, self.system_mode);
        self.refresh();
    }

    /// Handles one message.
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick(now) => {
                let changes = self.shell.poll(now);
                if changes.contains(Changes::CONFIG) || changes.contains(Changes::DEVICES) {
                    self.sync_texts();
                }
                self.sync_after_command();
                Task::none()
            }
            Message::Key(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) if self.editing.is_some() => self.update(Message::ChannelNameCancel),
            Message::Key(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) if self.settings_open || self.about_open => {
                self.settings_open = false;
                self.about_open = false;
                Task::none()
            }
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                let action = entry_for(&self.table, &key, modifiers)
                    .filter(|entry| entry.enabled_when.is_enabled(self.shell.state()))
                    .map(|entry| entry.action.clone());
                match action {
                    Some(Action::Command(command)) => self.run(command),
                    Some(Action::ToggleFollowLatest) => self.update(Message::ToggleFollow),
                    Some(Action::ShowLog) => self.update(Message::ShowLog),
                    Some(Action::OpenFile) => self.update(Message::OpenFile),
                    Some(Action::Settings) => self.update(Message::OpenSettings),
                    Some(Action::About) => self.update(Message::ShowAbout),
                    Some(Action::UiScaleUp) => self.update(Message::ScaleStep(1)),
                    Some(Action::UiScaleDown) => self.update(Message::ScaleStep(-1)),
                    Some(Action::SaveFile) => self.update(Message::SaveFile),
                    Some(Action::SaveFileAs) => self.update(Message::SaveFileAs),
                    Some(Action::PlaceMarker) => {
                        self.update(Message::MarkerAction(MarkerAction::Place))
                    }
                    Some(Action::PreviousMarker) => {
                        self.update(Message::MarkerAction(MarkerAction::Previous))
                    }
                    Some(Action::NextMarker) => {
                        self.update(Message::MarkerAction(MarkerAction::Next))
                    }
                    Some(Action::RemoveSelectedMarker) => {
                        self.update(Message::MarkerAction(MarkerAction::RemoveSelected))
                    }
                    Some(Action::AddDecoder) => self.update(Message::AddDecoderMenu),
                    Some(Action::ToggleSnap) => {
                        self.update(Message::MarkerAction(MarkerAction::ToggleSnap))
                    }
                    _ => Task::none(),
                }
            }
            Message::EditChannelName(channel, top) => {
                let original = self
                    .shell
                    .state()
                    .presentation
                    .channels
                    .iter()
                    .find(|c| c.index == channel)
                    .map(|c| c.name.clone())
                    .unwrap_or_default();
                self.editing = Some(ChannelEdit {
                    channel,
                    top,
                    text: original.clone(),
                    original,
                });
                Task::batch([
                    iced::widget::operation::focus(NAME_EDITOR),
                    iced::widget::operation::select_all(NAME_EDITOR),
                ])
            }
            Message::MoveLane { from, to } => match self.presentation_move(from, to) {
                Some((from, to)) => self.run(Command::MoveChannel { from, to }),
                None => Task::none(),
            },
            Message::SidePanel(view) => self.run(Command::SetSidePanel(view)),
            Message::ChannelNameInput(text) => {
                if let Some(edit) = &mut self.editing {
                    edit.text = text;
                }
                Task::none()
            }
            Message::ChannelNameAccept => match self.editing.take() {
                Some(edit) if edit.text != edit.original => {
                    let task = self.run(Command::RenameChannel {
                        channel: edit.channel,
                        name: edit.text,
                    });
                    self.sync_names();
                    task
                }
                _ => Task::none(),
            },
            Message::ChannelNameCancel => {
                self.editing = None;
                Task::none()
            }
            Message::Key(_) | Message::Noop => Task::none(),
            Message::ShowLog => {
                self.open_menu = None;
                if let Some(path) = self.shell.log_location() {
                    open_folder(path);
                }
                Task::none()
            }
            Message::MarkerAction(action) => {
                self.open_menu = None;
                self.marker_action(action)
            }
            Message::MarkerName(id, name) => {
                self.marker_names.insert(id, name.clone());
                self.run(Command::RenameMarker { id, name })
            }
            Message::MarkerMeasure(slot, id) => {
                let mut pair = self.shell.state().markers.selected;
                if let Some(entry) = pair.get_mut(slot) {
                    *entry = Some(id);
                }
                self.run(Command::SelectMarkers {
                    first: pair[0],
                    second: pair[1],
                })
            }
            Message::OpenSettings => {
                self.open_menu = None;
                self.about_open = false;
                self.memory_text =
                    (self.shell.state().settings.memory_limit_bytes >> 20).to_string();
                self.decoder_memory_text =
                    (self.shell.state().settings.decoder_memory_limit_bytes >> 20).to_string();
                self.settings_open = true;
                Task::none()
            }
            Message::CloseSettings => {
                self.settings_open = false;
                Task::none()
            }
            Message::ShowAbout => {
                self.open_menu = None;
                self.settings_open = false;
                self.about_open = true;
                Task::none()
            }
            Message::CloseAbout => {
                self.about_open = false;
                Task::none()
            }
            Message::LanguagePicked(choice) => self.run(Command::SetLanguage(choice.value)),
            Message::ThemePicked(choice) => self.run(Command::SetTheme(choice.value)),
            Message::ScaleStep(direction) => {
                let current = self.shell.state().settings.ui_scale_percent;
                let next = if direction > 0 {
                    SCALE_STEPS.iter().copied().find(|s| *s > current)
                } else {
                    SCALE_STEPS.iter().rev().copied().find(|s| *s < current)
                };
                match next {
                    Some(percent) => self.run(Command::SetUiScale(percent)),
                    None => Task::none(),
                }
            }
            Message::MemoryText(text) => {
                self.memory_text = text;
                match self.memory_text.trim().parse::<u64>() {
                    Ok(mib) if self.memory_limit_ok(mib) => {
                        self.run(Command::SetMemoryLimit(mib << 20))
                    }
                    _ => Task::none(),
                }
            }
            Message::DecoderMemoryText(text) => {
                self.decoder_memory_text = text;
                match self.decoder_memory_text.trim().parse::<u64>() {
                    Ok(mib) if Self::decoder_memory_ok(mib) => {
                        self.run(Command::SetDecoderMemoryLimit(mib << 20))
                    }
                    _ => Task::none(),
                }
            }
            Message::WindowEvent(event) => {
                match event {
                    window::Event::Resized(size) => {
                        self.window_rect[2] = size.width.round() as i32;
                        self.window_rect[3] = size.height.round() as i32;
                    }
                    window::Event::Moved(point) => {
                        self.window_rect[0] = point.x.round() as i32;
                        self.window_rect[1] = point.y.round() as i32;
                    }
                    _ => return Task::none(),
                }
                let [x, y, width, height] = self.window_rect;
                if width > 0 && height > 0 {
                    let geometry = la_host::settings::WindowGeometry {
                        x,
                        y,
                        width: u32::try_from(width).unwrap_or(0),
                        height: u32::try_from(height).unwrap_or(0),
                        maximized: false,
                    };
                    let _ = self.shell.apply(Command::SetWindowGeometry(geometry));
                }
                Task::none()
            }
            Message::OpenFile => {
                self.open_menu = None;
                self.choose_file(FileIntent::Open)
            }
            Message::SaveFile => {
                self.open_menu = None;
                if self.shell.state().session_path.is_some() {
                    self.run(Command::SaveSession)
                } else {
                    self.choose_file(FileIntent::SaveAs)
                }
            }
            Message::SaveFileAs => {
                self.open_menu = None;
                self.choose_file(FileIntent::SaveAs)
            }
            Message::FileChosen(intent, Some(path)) => match intent {
                FileIntent::Open => self.run(Command::OpenSession(path)),
                FileIntent::SaveAs => self.run(Command::SaveSessionAs(path)),
            },
            // Giving up the chooser leaves everything as it was (a question about unsaved data
            // stays open: the user answers it).
            Message::FileChosen(_, None) => Task::none(),
            Message::ToggleMessages => {
                self.messages_open = !self.messages_open;
                Task::none()
            }
            Message::ToggleFollow => {
                self.open_menu = None;
                let follow = !self.shell.state().follow_latest;
                self.run(Command::SetFollowLatest(follow))
            }
            Message::ChannelName(channel, name) => {
                self.names.insert(channel, name.clone());
                self.run(Command::RenameChannel { channel, name })
            }
            Message::ChannelVisible(channel, visible) => {
                self.run(Command::SetChannelVisible { channel, visible })
            }
            Message::ChannelColour(channel) => {
                let next = self
                    .shell
                    .state()
                    .presentation
                    .channels
                    .iter()
                    .find(|style| style.index == channel)
                    .map(|style| match style.colour {
                        Colour::Palette(index) => {
                            Colour::Palette(index.saturating_add(1) % PALETTE_SIZE)
                        }
                        Colour::Rgb(..) => Colour::Palette(channel % PALETTE_SIZE),
                    });
                match next {
                    Some(colour) => self.run(Command::SetChannelColour { channel, colour }),
                    None => Task::none(),
                }
            }
            Message::ChannelMove(position, direction) => {
                let to = if direction < 0 {
                    position.saturating_sub(1)
                } else {
                    position.saturating_add(1)
                };
                self.run(Command::MoveChannel { from: position, to })
            }
            Message::AreaResized(width) => {
                let pixels = width.max(1.0) as u32;
                self.run(Command::SetViewSize(ViewSize { pixels, scale: 1.0 }))
            }
            Message::Run(command) => {
                self.open_menu = None;
                self.run(command)
            }
            Message::AddDecoderMenu => {
                self.open_menu = None;
                let catalogue = &self.shell.state().decoders.catalogue;
                match catalogue.as_slice() {
                    [only] => {
                        let decoder = only.id.clone();
                        self.run(Command::AddDecoder { decoder })
                    }
                    _ => Task::none(),
                }
            }
            Message::DecoderSourcePicked(id, choice) => self.run(Command::StackDecoder {
                id,
                source: choice.value,
            }),
            Message::AddDecoderPicked(choice) => self.run(Command::AddDecoder {
                decoder: choice.value,
            }),
            Message::DecoderName(id, name) => {
                self.decoder_names.insert(id, name.clone());
                self.run(Command::RenameDecoder { id, name })
            }
            Message::DecoderChannelPicked(id, role, choice) => {
                self.run(Command::SetDecoderChannel {
                    id,
                    role,
                    channel: choice.value,
                })
            }
            Message::DecoderOptionPicked(id, option, choice) => {
                self.run(Command::SetDecoderOption {
                    id,
                    option,
                    value: Value::Choice(choice.value),
                })
            }
            Message::DecoderOptionText(id, option, text) => {
                let parsed = text.trim().parse::<i64>().ok();
                self.option_texts.insert((id, option.clone()), text);
                match parsed {
                    Some(number) => self.run(Command::SetDecoderOption {
                        id,
                        option,
                        value: Value::Integer(number),
                    }),
                    None => Task::none(),
                }
            }
            Message::DecoderGenerator(id, key, value) => {
                let Some(mut setup) = self
                    .shell
                    .state()
                    .decoders
                    .instance(id)
                    .map(|i| i.setup.clone())
                else {
                    return Task::none();
                };
                setup.generator.insert(key.to_owned(), value);
                self.run(Command::ConfigureDecoder { id, setup })
            }
            Message::DecoderRateText(id, text) => {
                let parsed = text.trim().parse::<i64>().ok();
                self.option_texts
                    .insert((id, "error_rate".to_owned()), text);
                match parsed {
                    Some(rate) => self.update(Message::DecoderGenerator(id, "error_rate", rate)),
                    None => Task::none(),
                }
            }
            Message::DecoderOptionFlag(id, option, on) => self.run(Command::SetDecoderOption {
                id,
                option,
                value: Value::Boolean(on),
            }),
            Message::MenuToggled(index) => {
                self.open_menu = if self.open_menu == Some(index) {
                    None
                } else {
                    Some(index)
                };
                Task::none()
            }
            Message::MenuClosed => {
                self.open_menu = None;
                Task::none()
            }
            Message::DevicePicked(choice) => {
                let task = self.run(Command::SelectDevice(choice.id));
                self.sync_texts();
                task
            }
            Message::RateText(text) => {
                self.rate_text.clone_from(&text);
                match text.trim().parse::<u64>() {
                    Ok(hz) => self.run(Command::EditConfig(ConfigEdit::SampleRate(hz))),
                    Err(_) => Task::none(),
                }
            }
            Message::LengthText(text) => {
                self.length_text.clone_from(&text);
                match text.trim().parse::<u64>() {
                    Ok(samples) => self.run(Command::EditConfig(ConfigEdit::Length(samples))),
                    Err(_) => Task::none(),
                }
            }
            Message::SeedText(text) => {
                self.seed_text.clone_from(&text);
                let seed = text.trim().parse::<u64>().ok();
                if text.trim().is_empty() || seed.is_some() {
                    self.run(Command::EditConfig(ConfigEdit::Seed(seed)))
                } else {
                    Task::none()
                }
            }
            Message::ProfilePicked(choice) => {
                self.run(Command::EditConfig(ConfigEdit::Profile(choice.profile)))
            }
            Message::RealtimeToggled(on) => self.run(Command::EditConfig(ConfigEdit::Realtime(on))),
            Message::SystemTheme(mode) => {
                self.system_mode = mode;
                self.sync_after_command();
                Task::none()
            }
            Message::CloseRequested => self.run(Command::RequestClose),
        }
    }

    // ---------------------------------------------------------------- view

    fn menu_bar(&self) -> Element<'_, Message> {
        let mut titles: Vec<&'static str> = Vec::new();
        for entry in &self.table {
            if !titles.contains(&entry.menu[0]) {
                titles.push(entry.menu[0]);
            }
        }
        let mut bar = row![].spacing(4).padding([2, 6]);
        for (index, title) in titles.iter().enumerate() {
            bar = bar.push(
                button(text(self.t(title)))
                    .style(button::text)
                    .padding([3, 10])
                    .on_press(Message::MenuToggled(index)),
            );
        }
        bar.into()
    }

    /// The open menu as a column of entries (shown above the content).
    fn open_menu_view(&self) -> Option<Element<'_, Message>> {
        let index = self.open_menu?;
        let mut titles: Vec<&'static str> = Vec::new();
        for entry in &self.table {
            if !titles.contains(&entry.menu[0]) {
                titles.push(entry.menu[0]);
            }
        }
        let title = titles.get(index)?;
        let mut items = column![].spacing(1).padding(4);
        for entry in self.table.iter().filter(|e| e.menu[0] == *title) {
            let enabled = entry.enabled_when.is_enabled(self.shell.state());
            let label = match entry.accelerator {
                Some(a) => format!("{}    {}", self.t(entry.menu[1]), describe(a)),
                None => self.t(entry.menu[1]),
            };
            let mut item = button(text(label))
                .style(button::text)
                .padding([4, 12])
                .width(Length::Shrink);
            if enabled {
                match &entry.action {
                    Action::Command(command) => {
                        item = item.on_press(Message::Run(command.clone()));
                    }
                    Action::ToggleFollowLatest => item = item.on_press(Message::ToggleFollow),
                    Action::ShowLog => item = item.on_press(Message::ShowLog),
                    Action::OpenFile => item = item.on_press(Message::OpenFile),
                    Action::Settings => item = item.on_press(Message::OpenSettings),
                    Action::About => item = item.on_press(Message::ShowAbout),
                    Action::UiScaleUp => item = item.on_press(Message::ScaleStep(1)),
                    Action::UiScaleDown => item = item.on_press(Message::ScaleStep(-1)),
                    Action::SaveFile => item = item.on_press(Message::SaveFile),
                    Action::SaveFileAs => item = item.on_press(Message::SaveFileAs),
                    Action::PlaceMarker => {
                        item = item.on_press(Message::MarkerAction(MarkerAction::Place));
                    }
                    Action::PreviousMarker => {
                        item = item.on_press(Message::MarkerAction(MarkerAction::Previous));
                    }
                    Action::NextMarker => {
                        item = item.on_press(Message::MarkerAction(MarkerAction::Next));
                    }
                    Action::RemoveSelectedMarker => {
                        item = item.on_press(Message::MarkerAction(MarkerAction::RemoveSelected));
                    }
                    Action::ToggleSnap => {
                        item = item.on_press(Message::MarkerAction(MarkerAction::ToggleSnap));
                    }
                    Action::AddDecoder => item = item.on_press(Message::AddDecoderMenu),
                }
            }
            items = items.push(item);
        }
        if *title == "menu-file" {
            let recent = self.shell.recent_files();
            if !recent.is_empty() {
                items = items.push(text(self.t("menu-file-recent")).size(12));
                for path in recent {
                    let label = path.file_name().map_or_else(
                        || path.display().to_string(),
                        |n| n.to_string_lossy().into_owned(),
                    );
                    let mut item = button(text(label).size(13))
                        .style(button::text)
                        .padding([3, 20])
                        .width(Length::Shrink);
                    if self.shell.state().operation.is_none() {
                        item = item.on_press(Message::Run(Command::OpenSession(path)));
                    }
                    items = items.push(item);
                }
            }
        }
        let offset = 6.0 + index as f32 * 90.0;
        Some(
            container(container(items).style(container::bordered_box))
                .padding(iced::Padding {
                    top: 30.0,
                    left: offset,
                    ..iced::Padding::ZERO
                })
                .into(),
        )
    }

    /// The toolbar: Start and Stop, and what keeps Start from working.
    fn toolbar(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let idle = state.activity == Activity::Idle;
        let localizer = self.shell.localizer();
        let start = if idle {
            let mut b = button(text(self.t("toolbar-start")));
            if state.config_problems.is_empty() && state.selected_device.is_some() {
                b = b.on_press(Message::Run(Command::StartCapture));
            }
            b
        } else {
            button(text(self.t("toolbar-stop"))).on_press(Message::Run(Command::StopCapture))
        };
        let mut toolbar = column![container(row![start]).padding([6, 8])];
        // "No channel is chosen" needs no message: the flags in the channel list show it, and
        // Start stays unavailable.
        for problem in state
            .config_problems
            .iter()
            .filter(|p| p.setting != la_host::devices::Setting::Channels)
        {
            toolbar = toolbar.push(
                container(
                    text(localizer.error_message_for_code(problem.code))
                        .size(13)
                        .style(text::danger),
                )
                .padding([0, 10]),
            );
        }
        toolbar.into()
    }

    /// The Capture view: everything about the next capture.
    fn capture_panel(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let idle = state.activity == Activity::Idle;
        let devices: Vec<DeviceChoice> = state
            .devices
            .iter()
            .map(|d| DeviceChoice {
                id: d.id.clone(),
                label: if d.simulated {
                    format!("{} ({})", d.name, self.t("status-simulated"))
                } else {
                    d.name.clone()
                },
            })
            .collect();
        let selected = devices
            .iter()
            .find(|c| Some(&c.id) == state.selected_device.as_ref())
            .cloned();
        let profiles: Vec<ProfileChoice> = [
            (SignalProfile::Sparse, "profile-sparse"),
            (SignalProfile::Normal, "profile-normal"),
            (SignalProfile::Dense, "profile-dense"),
        ]
        .into_iter()
        .map(|(profile, key)| ProfileChoice {
            profile,
            label: self.t(key),
        })
        .collect();
        let profile = profiles
            .iter()
            .find(|p| p.profile == state.config.profile)
            .cloned();

        let mut device_picker = pick_list(devices, selected, Message::DevicePicked);
        let mut profile_picker = pick_list(profiles, profile, Message::ProfilePicked);
        let mut rate = text_input("", &self.rate_text).width(Fill);
        let mut length = text_input("", &self.length_text).width(Fill);
        let mut seed = text_input(&self.t("toolbar-seed-random"), &self.seed_text).width(Fill);
        let mut realtime = checkbox(state.config.realtime).label(self.t("toolbar-realtime"));
        if idle {
            device_picker = device_picker.on_open(Message::Noop);
            rate = rate.on_input(Message::RateText);
            length = length.on_input(Message::LengthText);
            seed = seed.on_input(Message::SeedText);
            realtime = realtime.on_toggle(Message::RealtimeToggled);
        }
        let _ = &mut profile_picker;
        let mut view = column![
            text(self.t("panel-capture")).size(14),
            labelled(self.t("toolbar-device"), device_picker.width(Fill).into()),
            labelled(self.t("toolbar-rate"), rate.into()),
            labelled(self.t("toolbar-length"), length.into()),
            labelled(self.t("toolbar-seed"), seed.into()),
            labelled(self.t("toolbar-profile"), profile_picker.width(Fill).into()),
            realtime,
        ]
        .spacing(8)
        .padding(iced::Padding {
            top: 6.0,
            right: 18.0,
            bottom: 6.0,
            left: 6.0,
        });
        // For now the flags of the channels are the panel that was there before; the next step
        // replaces it with the buttons of the capture view.
        view = view.push(self.channel_panel());
        scrollable(view).height(Fill).into()
    }

    /// The panel with the markers and the measurement between two of them.
    fn markers_panel(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let localizer = self.shell.localizer();
        let mut list = column![].spacing(4).padding(iced::Padding {
            top: 6.0,
            right: 18.0,
            bottom: 6.0,
            left: 6.0,
        });
        list = list.push(text(self.t("panel-markers")).size(14));
        for (position, marker) in state.markers.list.iter().enumerate() {
            let default_label = localizer.message(
                &Key::from_static("marker-default"),
                &Args::new().uint("number", u64::from(marker.id)),
            );
            let name = self.marker_names.get(&marker.id).map_or("", String::as_str);
            let id = marker.id;
            let measured = |slot: usize, key: &'static str, label: &'static str| {
                let chosen = state.markers.selected.get(slot).copied().flatten() == Some(id);
                tooltip(
                    button(text(label).size(11))
                        .padding(3)
                        .style(if chosen {
                            button::primary
                        } else {
                            button::text
                        })
                        .on_press(Message::MarkerMeasure(slot, id)),
                    text(self.t(key)),
                    tooltip::Position::Bottom,
                )
            };
            let _ = position;
            list = list.push(
                row![
                    text_input(&default_label, name)
                        .size(13)
                        .width(Fill)
                        .on_input(move |t| Message::MarkerName(id, t)),
                    measured(0, "marker-measure-first", "1"),
                    measured(1, "marker-measure-second", "2"),
                    tooltip(
                        button(text("\u{21e5}").size(12))
                            .style(button::text)
                            .padding(3)
                            .on_press(Message::Run(Command::GoToMarker(id))),
                        text(self.t("marker-go")),
                        tooltip::Position::Bottom,
                    ),
                    tooltip(
                        button(text("\u{d7}").size(12))
                            .style(button::text)
                            .padding(3)
                            .on_press(Message::Run(Command::RemoveMarker(id))),
                        text(self.t("marker-remove")),
                        tooltip::Position::Bottom,
                    ),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            );
        }
        match self.shell.measurement_text() {
            Some(measurement) => {
                list = list.push(text(format!(
                    "{}: {}",
                    self.t("measure-distance"),
                    measurement.distance
                )));
                if let Some(frequency) = measurement.frequency {
                    list = list.push(text(format!(
                        "{}: {frequency}",
                        self.t("measure-frequency")
                    )));
                }
            }
            None if state.capture.is_some() => {
                list = list.push(text(self.t("measure-pick")).size(12));
            }
            None => {}
        }
        container(scrollable(list))
            .width(Fill)
            .height(Length::FillPortion(1))
            .style(container::bordered_box)
            .into()
    }

    /// The panel of the decoders: the instances, their channels and options, the state of each
    /// pass and the selected annotation.
    fn decoders_panel(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let localizer = self.shell.localizer();
        let mut list = column![].spacing(6).padding(iced::Padding {
            top: 6.0,
            right: 18.0,
            bottom: 6.0,
            left: 6.0,
        });
        list = list.push(text(self.t("panel-decoders")).size(14));
        if state.capture.is_none() {
            list = list.push(text(self.t("decoders-need-capture")).size(12));
        } else {
            let catalogue = &state.decoders.catalogue;
            if catalogue.len() == 1 {
                list = list.push(
                    button(text(self.t("decoders-add")).size(13))
                        .padding([3, 8])
                        .on_press(Message::AddDecoderMenu),
                );
            } else {
                let choices: Vec<Choice<String>> = catalogue
                    .iter()
                    .map(|entry| Choice {
                        value: entry.id.clone(),
                        label: entry.name.clone(),
                    })
                    .collect();
                list = list.push(
                    pick_list(choices, None::<Choice<String>>, Message::AddDecoderPicked)
                        .placeholder(self.t("decoders-add"))
                        .text_size(13),
                );
            }
            if state.decoders.instances.is_empty() {
                list = list.push(text(self.t("decoders-empty")).size(12));
            }
        }
        for instance in &state.decoders.instances {
            list = list.push(self.decoder_block(instance));
        }
        if let Some(refusal) = &state.decoders.refusal {
            list = list.push(
                text(localizer.error_message_for_code(refusal.code))
                    .size(12)
                    .style(text::danger),
            );
        }
        if let Some(selection) = &state.decoders.selection {
            let details = &selection.details;
            list = list.push(text(self.t("decoder-selected")).size(13));
            list = list.push(text(details.text.clone()).size(13));
            list = list.push(
                text(
                    localizer.message(
                        &Key::from_static("decoder-detail-position"),
                        &Args::new()
                            .uint("first", details.start)
                            .uint("second", details.end),
                    ),
                )
                .size(12),
            );
            list = list.push(
                text(localizer.message(
                    &Key::from_static("decoder-detail-length"),
                    &Args::new().text("value", &details.duration),
                ))
                .size(12),
            );
            if details.error {
                list = list.push(
                    text(self.t("decoder-detail-error"))
                        .size(12)
                        .style(text::danger),
                );
            }
        }
        container(scrollable(list))
            .width(Fill)
            .height(Length::FillPortion(4))
            .style(container::bordered_box)
            .into()
    }

    /// One decoder instance: name, lane flag, state, channels and options.
    fn decoder_block(&self, instance: &la_host::decoders::InstanceView) -> Element<'_, Message> {
        let state = self.shell.state();
        let localizer = self.shell.localizer();
        let id = instance.id;
        let entry = state
            .decoders
            .catalogue
            .iter()
            .find(|c| c.id == instance.setup.decoder);
        let default_name = entry.map_or_else(String::new, |e| e.name.clone());
        let name = self
            .decoder_names
            .get(&id)
            .map_or(instance.setup.name.as_str(), String::as_str);
        let state_key = match instance.state {
            InstanceState::Waiting => "decoder-state-waiting",
            InstanceState::Running => "decoder-state-running",
            InstanceState::Finished => "decoder-state-finished",
            InstanceState::Cancelled => "decoder-state-cancelled",
            InstanceState::Failed => "decoder-state-failed",
        };
        let mut block = column![
            row![
                text_input(&default_name, name)
                    .size(13)
                    .width(Fill)
                    .on_input(move |t| Message::DecoderName(id, t)),
                tooltip(
                    button(text("\u{d7}").size(12))
                        .style(button::text)
                        .padding(3)
                        .on_press(Message::Run(Command::RemoveDecoder(id))),
                    text(self.t("decoder-remove")),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
        ]
        .spacing(3);
        block = block.push(
            checkbox(instance.setup.lane_visible)
                .label(self.t("decoder-lane-show"))
                .size(14)
                .on_toggle(move |visible| Message::Run(Command::ShowDecoderLane { id, visible })),
        );
        let mut status = self.t(state_key);
        if instance.state == InstanceState::Running && instance.length > 0 {
            let percent = instance
                .covered
                .saturating_mul(100)
                .checked_div(instance.length)
                .unwrap_or(0);
            status = format!("{status} {percent}%");
        }
        block = block.push(
            text(format!(
                "{status}  \u{b7}  {}",
                localizer.message(
                    &Key::from_static("decoder-annotations"),
                    &Args::new().uint("number", instance.annotations),
                )
            ))
            .size(12),
        );
        if instance.state == InstanceState::Running && instance.length > 0 {
            block = block.push(progress_bar(
                0.0..=1.0,
                instance.covered as f32 / instance.length as f32,
            ));
        }
        if !instance.ready {
            block = block.push(text(self.t("decoder-needs-channels")).size(12));
        } else {
            // Stop a pass that runs; run a new one when nothing does.
            let running = instance.state == InstanceState::Running;
            block = block.push(
                button(
                    text(self.t(if running {
                        "decoder-cancel"
                    } else {
                        "decoder-run"
                    }))
                    .size(12),
                )
                .padding([2, 8])
                .on_press(Message::Run(if running {
                    Command::CancelDecoder(id)
                } else {
                    Command::RunDecoder(id)
                })),
            );
        }
        if let Some(code) = instance.fault {
            block = block.push(
                text(localizer.error_message_for_code(code))
                    .size(12)
                    .style(text::danger),
            );
        } else if instance.incomplete {
            block = block.push(text(self.t("decoder-incomplete")).size(12));
        }
        if let Some(entry) = entry {
            // Where it reads from, when there is a choice (a stacked decoder reads another one).
            let sources = self.shell.decoder_sources(id);
            if sources.len() > 1 || entry.reads_items {
                let label_of = |source: &Source| match source {
                    Source::Capture => self.t("decoder-source-capture"),
                    Source::Instance(other) => state
                        .decoders
                        .instance(*other)
                        .map(|i| {
                            if i.setup.name.is_empty() {
                                state
                                    .decoders
                                    .catalogue
                                    .iter()
                                    .find(|c| c.id == i.setup.decoder)
                                    .map_or_else(String::new, |c| c.name.clone())
                            } else {
                                i.setup.name.clone()
                            }
                        })
                        .unwrap_or_default(),
                };
                let choices: Vec<Choice<Source>> = sources
                    .iter()
                    .map(|source| Choice {
                        value: *source,
                        label: label_of(source),
                    })
                    .collect();
                let selected = choices
                    .iter()
                    .find(|c| c.value == instance.setup.source)
                    .cloned();
                block = block.push(labelled(
                    self.t("decoder-source"),
                    pick_list(choices, selected, move |choice| {
                        Message::DecoderSourcePicked(id, choice)
                    })
                    .text_size(13)
                    .into(),
                ));
            }
            let mut channel_choices: Vec<Choice<Option<u8>>> = vec![Choice {
                value: None,
                label: self.t("decoder-channel-none"),
            }];
            for style in &state.presentation.channels {
                let label = if style.name.is_empty() {
                    localizer.message(
                        &Key::from_static("channel-default"),
                        &Args::new().uint("number", u64::from(style.index)),
                    )
                } else {
                    style.name.clone()
                };
                channel_choices.push(Choice {
                    value: Some(style.index),
                    label,
                });
            }
            for signal in &entry.signals {
                let current = instance.setup.channels.get(&signal.role).copied();
                let selected = channel_choices.iter().find(|c| c.value == current).cloned();
                let role = signal.role.clone();
                block = block.push(labelled(
                    signal.label.clone(),
                    pick_list(channel_choices.clone(), selected, move |choice| {
                        Message::DecoderChannelPicked(id, role.clone(), choice)
                    })
                    .text_size(13)
                    .into(),
                ));
            }
            for option in &entry.options {
                let option_id = option.id.clone();
                let current = state
                    .decoders
                    .instance(id)
                    .and_then(|i| i.setup.options.get(&option.id).cloned());
                let field: Element<'_, Message> = match &option.kind {
                    CatalogueOptionKind::Choice { choices, default } => {
                        let all: Vec<Choice<String>> = choices
                            .iter()
                            .map(|(value, label)| Choice {
                                value: value.clone(),
                                label: label.clone(),
                            })
                            .collect();
                        let wanted = match current {
                            Some(Value::Choice(v)) => v,
                            _ => default.clone(),
                        };
                        let selected = all.iter().find(|c| c.value == wanted).cloned();
                        pick_list(all, selected, move |choice| {
                            Message::DecoderOptionPicked(id, option_id.clone(), choice)
                        })
                        .text_size(13)
                        .into()
                    }
                    CatalogueOptionKind::Integer { default, .. } => {
                        let shown = self
                            .option_texts
                            .get(&(id, option.id.clone()))
                            .cloned()
                            .unwrap_or_else(|| match current {
                                Some(Value::Integer(v)) => v.to_string(),
                                _ => default.to_string(),
                            });
                        text_input("", &shown)
                            .size(13)
                            .width(110)
                            .on_input(move |t| Message::DecoderOptionText(id, option_id.clone(), t))
                            .into()
                    }
                    CatalogueOptionKind::Boolean { default } => {
                        let on = match current {
                            Some(Value::Boolean(v)) => v,
                            _ => *default,
                        };
                        checkbox(on)
                            .on_toggle(move |v| {
                                Message::DecoderOptionFlag(id, option_id.clone(), v)
                            })
                            .into()
                    }
                };
                block = block.push(labelled(option.label.clone(), field));
            }
            // A simulated device makes signals that are valid for the decoder, with errors on
            // request.
            let simulated = state
                .selected_device
                .as_ref()
                .and_then(|id| state.devices.iter().find(|d| d.id == *id))
                .is_some_and(|d| d.simulated);
            if simulated && entry.has_generator {
                let errors = instance
                    .setup
                    .generator
                    .get(la_decoders::generators::clocked_bits::OPTION_ERRORS)
                    .copied()
                    .unwrap_or(0)
                    != 0;
                block = block.push(
                    checkbox(errors)
                        .label(self.t("decoder-generator-errors"))
                        .size(14)
                        .on_toggle(move |on| {
                            Message::DecoderGenerator(
                                id,
                                la_decoders::generators::clocked_bits::OPTION_ERRORS,
                                i64::from(on),
                            )
                        }),
                );
                if errors {
                    let rate = instance
                        .setup
                        .generator
                        .get(la_decoders::generators::clocked_bits::OPTION_ERROR_RATE)
                        .copied()
                        .unwrap_or(la_decoders::generators::clocked_bits::DEFAULT_ERROR_RATE);
                    let shown = self
                        .option_texts
                        .get(&(id, "error_rate".to_owned()))
                        .cloned()
                        .unwrap_or_else(|| rate.to_string());
                    block = block.push(labelled(
                        self.t("decoder-generator-rate"),
                        text_input("", &shown)
                            .size(13)
                            .width(110)
                            .on_input(move |t| Message::DecoderRateText(id, t))
                            .into(),
                    ));
                }
            }
        }
        container(block)
            .padding(4)
            .width(Fill)
            .style(container::rounded_box)
            .into()
    }

    /// The panel that lists the channels: show, name, colour and order.
    fn channel_panel(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let localizer = self.shell.localizer();
        let variant = self.variant;
        let mut list = column![].spacing(4).padding(iced::Padding {
            top: 6.0,
            right: 18.0,
            bottom: 6.0,
            left: 6.0,
        });
        list = list.push(text(self.t("panel-channels")).size(14));
        // The one flag of a channel shows it and makes the next capture record it; it is locked
        // only while a capture runs, because what that capture records cannot change.
        let can_flag = state.activity == Activity::Idle;
        let last = state.presentation.channels.len().saturating_sub(1);
        for (position, style) in state.presentation.channels.iter().enumerate() {
            let default_label = localizer.message(
                &Key::from_static("channel-default"),
                &Args::new().uint("number", u64::from(style.index)),
            );
            let name = self.names.get(&style.index).map_or("", String::as_str);
            let swatch = container(Space::new().width(14).height(14)).style({
                let colour = variant.channel(style.colour);
                move |_theme: &Theme| container::Style {
                    background: Some(colour.into()),
                    border: iced::Border {
                        radius: 3.0.into(),
                        ..iced::Border::default()
                    },
                    ..container::Style::default()
                }
            });
            let index = style.index;
            let line = row![
                checkbox(style.visible).on_toggle_maybe(
                    can_flag.then_some(move |v| Message::ChannelVisible(index, v)),
                ),
                text_input(&default_label, name)
                    .size(13)
                    .width(Fill)
                    .on_input(move |t| Message::ChannelName(index, t)),
                tooltip(
                    button(swatch)
                        .style(button::text)
                        .padding(4)
                        .on_press(Message::ChannelColour(index)),
                    text(self.t("channel-colour")),
                    tooltip::Position::Bottom,
                ),
                tooltip(
                    button(text("\u{25b2}").size(11))
                        .style(button::text)
                        .padding(3)
                        .on_press_maybe(
                            (position > 0).then_some(Message::ChannelMove(position, -1))
                        ),
                    text(self.t("channel-move-up")),
                    tooltip::Position::Bottom,
                ),
                tooltip(
                    button(text("\u{25bc}").size(11))
                        .style(button::text)
                        .padding(3)
                        .on_press_maybe(
                            (position < last).then_some(Message::ChannelMove(position, 1))
                        ),
                    text(self.t("channel-move-down")),
                    tooltip::Position::Bottom,
                ),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center);
            list = list.push(line);
        }
        list = list.push(
            button(text(self.t("channel-reset")).size(13))
                .style(button::secondary)
                .on_press(Message::Run(Command::ResetPresentation)),
        );
        container(scrollable(list))
            .width(Fill)
            .height(Length::FillPortion(3))
            .style(container::bordered_box)
            .into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let line = self.shell.status_line();
        let mut parts: Vec<String> = vec![line.activity];
        parts.extend(line.elapsed);
        parts.extend(line.samples);
        parts.extend(line.memory);
        parts.extend(line.end_reason);
        parts.extend(line.rate);
        if line.simulated {
            parts.push(self.t("status-simulated"));
        }
        if line.incomplete {
            parts.push(self.t("status-incomplete"));
        }
        if line.unsaved {
            parts.push(self.t("status-unsaved"));
        }
        container(text(parts.join("  \u{b7}  ")).size(13))
            .padding([4, 10])
            .width(Fill)
            .style(container::bordered_box)
            .into()
    }

    fn notice_line(&self, notice: &la_host::notices::Notice) -> String {
        let shown = self.shell.notice_text(notice);
        if notice.count > 1 {
            format!("{shown} (\u{d7}{})", notice.count)
        } else {
            shown
        }
    }

    /// The newest message with its buttons, and the list of recent messages when it is open.
    fn banner(&self) -> Option<Element<'_, Message>> {
        let state = self.shell.state();
        let notice = state.notices.iter().next_back()?;
        let id = notice.id;
        let total = state.notices.len();
        let mut buttons = row![].spacing(6).align_y(iced::Alignment::Center);
        if self.shell.log_location().is_some() {
            buttons = buttons.push(
                button(text(self.t("notice-log")).size(13))
                    .style(button::secondary)
                    .on_press(Message::ShowLog),
            );
        }
        if total > 1 {
            buttons = buttons.push(
                button(text(format!("{} ({total})", self.t("notice-list"))).size(13))
                    .style(button::secondary)
                    .on_press(Message::ToggleMessages),
            );
        }
        buttons = buttons.push(
            button(text(self.t("notice-dismiss")).size(13))
                .style(button::secondary)
                .on_press(Message::Run(Command::DismissNotice(id))),
        );
        if total > 1 {
            buttons = buttons.push(
                button(text(self.t("notice-dismiss-all")).size(13))
                    .style(button::secondary)
                    .on_press(Message::Run(Command::DismissAllNotices)),
            );
        }
        let mut banner = column![
            row![text(self.notice_line(notice)).width(Fill), buttons]
                .spacing(10)
                .align_y(iced::Alignment::Center)
        ]
        .spacing(4);
        if self.messages_open {
            let mut list = column![].spacing(2);
            for older in state.notices.iter().rev() {
                list = list.push(text(self.notice_line(older)).size(13));
            }
            banner = banner.push(scrollable(list).height(Length::Shrink));
        }
        Some(
            container(banner)
                .padding([4, 10])
                .width(Fill)
                .style(container::danger)
                .into(),
        )
    }

    /// The progress of a save or load, with a button to cancel it.
    fn progress_dialog(&self, progress: la_host::OperationView) -> Element<'_, Message> {
        let title = match progress.kind {
            la_host::OperationKind::Save => self.t("dialog-saving"),
            la_host::OperationKind::Load => self.t("dialog-opening"),
        };
        let total = progress.total_bytes.max(1) as f32;
        let body = column![
            text(title).size(18),
            progress_bar(0.0..=total, progress.done_bytes as f32).length(Length::Fixed(320.0)),
            button(text(self.t("dialog-cancel")))
                .style(button::secondary)
                .on_press(Message::Run(Command::CancelOperation)),
        ]
        .spacing(12)
        .max_width(420);
        container(container(body).padding(20).style(container::bordered_box))
            .center(Fill)
            .into()
    }

    fn unsaved_dialog(&self) -> Element<'_, Message> {
        // The question about removing a decoder that others read from is not about the capture.
        if let Some(la_host::Guarded::RemoveDecoder(_)) = self.shell.state().pending_decision {
            let body = column![
                text(self.t("dialog-remove-decoder-title")).size(18),
                text(self.t("dialog-remove-decoder-body")),
                row![
                    button(text(self.t("dialog-remove-decoder-confirm")))
                        .style(button::danger)
                        .on_press(Message::Run(Command::Decide(Decision::Discard))),
                    button(text(self.t("dialog-cancel")))
                        .style(button::secondary)
                        .on_press(Message::Run(Command::Decide(Decision::Cancel))),
                ]
                .spacing(8),
            ]
            .spacing(12)
            .max_width(420);
            return container(container(body).padding(20).style(container::bordered_box))
                .center(Fill)
                .into();
        }
        let body = column![
            text(self.t("dialog-unsaved-title")).size(18),
            text(self.t("dialog-unsaved-body")),
            row![
                button(text(self.t("dialog-save"))).on_press_maybe(
                    (self.shell.state().activity == Activity::Idle
                        && self.shell.state().operation.is_none())
                    .then_some(Message::Run(Command::Decide(Decision::Save)))
                ),
                button(text(self.t("dialog-discard")))
                    .style(button::danger)
                    .on_press(Message::Run(Command::Decide(Decision::Discard))),
                button(text(self.t("dialog-cancel")))
                    .style(button::secondary)
                    .on_press(Message::Run(Command::Decide(Decision::Cancel))),
            ]
            .spacing(8),
        ]
        .spacing(12)
        .max_width(420);
        container(container(body).padding(20).style(container::bordered_box))
            .center(Fill)
            .into()
    }

    /// The window content.
    pub fn view(&self) -> Element<'_, Message> {
        let signal = canvas(SignalView {
            plan: &self.plan,
            cache: &self.cache,
            variant: self.variant,
            localizer: self.shell.localizer(),
            selected: self
                .shell
                .state()
                .decoders
                .selection
                .as_ref()
                .map(|s| (s.instance, s.annotation)),
            editing: self.editing.is_some(),
        })
        .width(Fill)
        .height(Fill);
        // The editor of a channel name sits over the canvas, over the name it edits.
        let signal: Element<'_, Message> = match &self.editing {
            Some(edit) => {
                let placeholder = self.shell.text(
                    &Key::from_static("channel-default"),
                    &Args::new().uint("number", u64::from(edit.channel)),
                );
                let editor = text_input(&placeholder, &edit.text)
                    .id(NAME_EDITOR)
                    .size(13)
                    .padding(2)
                    .width(crate::lane_layout::GUTTER - 14.0)
                    .on_input(Message::ChannelNameInput)
                    .on_submit(Message::ChannelNameAccept);
                stack![
                    signal,
                    container(editor).padding(iced::Padding {
                        top: edit.top + 4.0,
                        left: 8.0,
                        ..iced::Padding::ZERO
                    })
                ]
                .into()
            }
            None => signal.into(),
        };

        let mut content = column![self.menu_bar(), self.toolbar()];
        if let Some(banner) = self.banner() {
            content = content.push(banner);
        }
        let side = self.shell.state().side_panel;
        let mut body = row![signal].height(Fill);
        let panel: Option<Element<'_, Message>> = match side {
            la_host::SidePanel::Capture => Some(self.capture_panel()),
            la_host::SidePanel::Decoders => Some(scrollable(self.decoders_panel()).into()),
            la_host::SidePanel::Markers => Some(scrollable(self.markers_panel()).into()),
            la_host::SidePanel::Hidden => None,
        };
        if let Some(panel) = panel {
            body = body.push(crate::side_panel::frame(panel));
        }
        body = body.push(crate::side_panel::strip(
            side,
            [
                self.t("panel-view-capture"),
                self.t("panel-view-decoders"),
                self.t("panel-view-markers"),
            ],
        ));
        let content = content.push(body).push(self.status_bar());

        let mut layers: Vec<Element<'_, Message>> = vec![content.into()];
        if let Some(menu) = self.open_menu_view() {
            layers.push(
                mouse_area(Space::new().width(Fill).height(Fill))
                    .on_press(Message::MenuClosed)
                    .into(),
            );
            layers.push(menu);
        }
        if let Some(progress) = self.shell.state().operation {
            layers.push(
                mouse_area(container(Space::new().width(Fill).height(Fill)).style(backdrop)).into(),
            );
            layers.push(self.progress_dialog(progress));
        } else if self.settings_open || self.about_open {
            layers.push(
                mouse_area(container(Space::new().width(Fill).height(Fill)).style(backdrop))
                    .on_press(if self.settings_open {
                        Message::CloseSettings
                    } else {
                        Message::CloseAbout
                    })
                    .into(),
            );
            layers.push(if self.settings_open {
                self.settings_dialog()
            } else {
                self.about_dialog()
            });
        } else if self.shell.state().pending_decision.is_some() {
            layers.push(
                mouse_area(container(Space::new().width(Fill).height(Fill)).style(backdrop)).into(),
            );
            layers.push(self.unsaved_dialog());
        }
        stack(layers).into()
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// The translucent layer behind a dialog: the window stays visible, dimmed.
fn backdrop(_theme: &Theme) -> container::Style {
    container::Style {
        background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.55).into()),
        ..container::Style::default()
    }
}

/// Opens a folder in the file manager of the system (best effort: nothing happens if there is
/// none).
fn open_folder(path: &std::path::Path) {
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(not(windows))]
    let program = "xdg-open";
    if let Err(error) = std::process::Command::new(program).arg(path).spawn() {
        tracing::warn!(target: "la_app", %error, "cannot open the folder");
    }
}

/// A small label above a field.
fn labelled<'a>(label: String, field: Element<'a, Message>) -> Element<'a, Message> {
    column![text(label).size(12), field].spacing(2).into()
}

/// "Ctrl+Shift+S" for an accelerator (the key names are the same in every language).
fn describe(accelerator: Accelerator) -> String {
    let mut text = String::new();
    if accelerator.ctrl {
        text.push_str("Ctrl+");
    }
    if accelerator.shift {
        text.push_str("Shift+");
    }
    if accelerator.alt {
        text.push_str("Alt+");
    }
    text.push_str(accelerator.key);
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::keyboard::{Key, Modifiers, key::Named};

    fn table() -> Vec<ShortcutEntry> {
        la_host::shortcuts::table()
    }

    /// Headless rendering creates a software renderer; doing that from several test threads at
    /// once crashed the Windows runner of the CI (STATUS_ACCESS_VIOLATION), so the tests that
    /// render take turns.
    fn render_turn() -> std::sync::MutexGuard<'static, ()> {
        static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
        TURN.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn f5_starts_and_shift_f5_stops() {
        let table = table();
        let start = entry_for(&table, &Key::Named(Named::F5), Modifiers::empty()).unwrap();
        assert_eq!(start.action, Action::Command(Command::StartCapture));
        let stop = entry_for(&table, &Key::Named(Named::F5), Modifiers::SHIFT).unwrap();
        assert_eq!(stop.action, Action::Command(Command::StopCapture));
    }

    #[test]
    fn letters_match_in_either_case_with_ctrl() {
        let table = table();
        for letter in ["q", "Q"] {
            let entry = entry_for(&table, &Key::Character(letter.into()), Modifiers::CTRL).unwrap();
            assert_eq!(entry.action, Action::Command(Command::Quit));
        }
        assert!(entry_for(&table, &Key::Character("q".into()), Modifiers::empty()).is_none());
        assert!(entry_for(&table, &Key::Unidentified, Modifiers::CTRL).is_none());
    }

    /// What the interface thread does while a decoder works on a very large capture: the longest
    /// call during decoding, a cancel, zooming and a language change, and the time of the frame plan
    /// with the full set of annotations. Like the measurement above it is run on purpose:
    /// `cargo test -p la-app --release measure_decoder_responsiveness -- --ignored --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check; run it on purpose"]
    fn measure_decoder_responsiveness() {
        use std::time::Duration;

        fn row(label: &str, v: &mut [Duration]) {
            v.sort_unstable();
            let at = |p: usize| {
                v.get(v.len().saturating_sub(1) * p / 100)
                    .copied()
                    .unwrap_or_default()
            };
            println!(
                "{label:<34} n={:<4} p50 {:>7.2} ms  p95 {:>7.2} ms  max {:>7.2} ms",
                v.len(),
                at(50).as_secs_f64() * 1e3,
                at(95).as_secs_f64() * 1e3,
                at(100).as_secs_f64() * 1e3
            );
        }

        let _turn = render_turn();
        let dir =
            std::env::temp_dir().join(format!("la-app-decoder-measure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        for edit in [
            ConfigEdit::Realtime(false),
            ConfigEdit::Channels(0xffff),
            ConfigEdit::SampleRate(100_000_000),
            ConfigEdit::Length(100_000_000),
            ConfigEdit::Seed(Some(5)),
            ConfigEdit::AcceptBudgetRisk(true),
        ] {
            let _ = app.update(Message::Run(Command::EditConfig(edit)));
        }
        let _ = app.update(Message::AreaResized(1900.0));
        // A first capture, so that a decoder can be added; the second one is decoded as it grows.
        let _ = app.update(Message::Run(Command::EditConfig(ConfigEdit::Length(
            1_000_000,
        ))));
        let _ = app.update(Message::Run(Command::StartCapture));
        while app.shell.state().activity != Activity::Idle {
            let _ = app.update(Message::Tick(Instant::now()));
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = app.update(Message::AddDecoderPicked(Choice {
            value: la_decoders::decoders::clocked_bits::ID.to_owned(),
            label: String::new(),
        }));
        let id = app.shell.state().decoders.instances[0].id;
        for (role, channel) in [("clock", 0_u8), ("data", 1)] {
            let _ = app.update(Message::DecoderChannelPicked(
                id,
                role.to_owned(),
                Choice {
                    value: Some(channel),
                    label: String::new(),
                },
            ));
        }
        let _ = app.update(Message::Run(Command::EditConfig(ConfigEdit::Length(
            100_000_000,
        ))));
        let mut decoding = Vec::new();
        let _ = app.update(Message::Run(Command::StartCapture));
        loop {
            let t = Instant::now();
            let _ = app.update(Message::Tick(Instant::now()));
            let _ = app.view();
            decoding.push(t.elapsed());
            let view = app.shell.state().decoders.instance(id).unwrap();
            if app.shell.state().activity == Activity::Idle
                && view.state != la_host::decoders::InstanceState::Running
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let annotations = app.shell.state().decoders.instance(id).unwrap().annotations;
        println!("{annotations} annotations over 100 M samples");
        row("during capture + decoding", &mut decoding);
        let mut plans = Vec::new();
        let mut views = Vec::new();
        for i in 0..300 {
            let message = match i % 3 {
                0 => Message::Run(Command::Zoom {
                    factor: la_host::ZoomFactor {
                        numerator: 4,
                        denominator: 5,
                    },
                    anchor: la_host::ZoomAnchor::Column(900),
                }),
                1 => Message::Run(Command::Pan { pixels: 200 }),
                _ => Message::Run(Command::Zoom {
                    factor: la_host::ZoomFactor {
                        numerator: 5,
                        denominator: 4,
                    },
                    anchor: la_host::ZoomAnchor::Column(400),
                }),
            };
            let t = Instant::now();
            let _ = app.update(message);
            plans.push(t.elapsed());
            let t = Instant::now();
            let _ = app.view();
            views.push(t.elapsed());
        }
        row("zoom and pan: update + plan", &mut plans);
        row("zoom and pan: widget tree", &mut views);
        let mut language = Vec::new();
        for l in [la_i18n::Language::Uk, la_i18n::Language::En] {
            let t = Instant::now();
            let _ = app.update(Message::Run(Command::SetLanguage(l)));
            let _ = app.view();
            language.push(t.elapsed());
        }
        row("language change", &mut language);
        // A cancel while a pass runs: set the setup anew, then cancel at once.
        let _ = app.update(Message::Run(Command::RunDecoder(id)));
        let t = Instant::now();
        let _ = app.update(Message::Run(Command::CancelDecoder(id)));
        while app.shell.state().decoders.instance(id).unwrap().state
            == la_host::decoders::InstanceState::Running
        {
            let _ = app.update(Message::Tick(Instant::now()));
            std::thread::sleep(Duration::from_millis(2));
        }
        println!("cancel took {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Runs a fast capture on the simulated device and returns the application showing it.
    fn app_with_capture(name: &str) -> App {
        let dir = std::env::temp_dir().join(format!("la-app-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        for edit in [
            ConfigEdit::Realtime(false),
            ConfigEdit::Channels(0b1111),
            ConfigEdit::SampleRate(1_000_000),
            ConfigEdit::Length(300_000),
            ConfigEdit::Seed(Some(3)),
        ] {
            let _ = app.update(Message::Run(Command::EditConfig(edit)));
        }
        let _ = app.update(Message::Run(Command::StartCapture));
        let deadline = Instant::now().checked_add(Duration::from_secs(20)).unwrap();
        while app.shell.state().activity != Activity::Idle {
            let _ = app.update(Message::Tick(Instant::now()));
            assert!(Instant::now() < deadline, "the capture did not end");
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = app.update(Message::AreaResized(1000.0));
        let _ = std::fs::remove_dir_all(&dir);
        app
    }

    #[test]
    fn the_view_renders_a_capture_headlessly() {
        let _turn = render_turn();
        let app = app_with_capture("render");
        assert!(!app.plan.channels.is_empty());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        let snapshot = ui.snapshot(&app.theme()).unwrap();
        // With LA_SNAPSHOT_DIR set the picture is written there for a person to look at.
        if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
            snapshot
                .matches_image(std::path::Path::new(&directory).join("capture"))
                .unwrap();
        }
    }

    #[test]
    fn the_legend_with_custom_heights_and_the_editor_open_renders_in_two_languages() {
        let _turn = render_turn();
        let mut app = app_with_capture("legend-editor");
        let _ = app.update(Message::Run(Command::SetChannelHeight {
            channel: 0,
            height: 90,
        }));
        let _ = app.update(Message::Run(Command::SetChannelHeight {
            channel: 1,
            height: 30,
        }));
        for language in [la_i18n::Language::En, la_i18n::Language::Uk] {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            let _ = app.update(Message::EditChannelName(0, 40.0));
            assert_eq!(app.editing.as_ref().map(|e| e.channel), Some(0));
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(800.0, 560.0),
                app.view(),
            );
            let shot = ui.snapshot(&app.theme()).unwrap();
            if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
                shot.matches_image(
                    std::path::Path::new(&directory).join(format!("legend-{}", language.tag())),
                )
                .unwrap();
            }
            drop(ui);
            let _ = app.update(Message::ChannelNameCancel);
            assert!(app.editing.is_none());
        }
    }

    #[test]
    fn the_side_panel_shows_one_view_at_a_time_or_none_in_every_state() {
        let _turn = render_turn();
        let mut app = app_with_capture("side-panel");
        // A text that only that view has: not a menu title and not in the other views.
        let marks = [
            (la_host::SidePanel::Capture, app.t("toolbar-rate")),
            (la_host::SidePanel::Decoders, app.t("decoders-empty")),
            (la_host::SidePanel::Markers, app.t("measure-pick")),
        ];
        for size in [
            iced::Size::new(1180.0, 720.0),
            iced::Size::new(800.0, 560.0),
        ] {
            for shown in [
                la_host::SidePanel::Capture,
                la_host::SidePanel::Decoders,
                la_host::SidePanel::Markers,
                la_host::SidePanel::Hidden,
            ] {
                let _ = app.update(Message::SidePanel(shown));
                assert_eq!(app.shell.state().side_panel, shown);
                let mut ui =
                    iced_test::Simulator::with_size(crate::fonts::settings(), size, app.view());
                for (view, text) in &marks {
                    assert_eq!(
                        ui.find(text.as_str()).is_ok(),
                        *view == shown,
                        "{shown:?} at {size:?}: the text of {view:?}"
                    );
                }
                let shot = ui.snapshot(&app.theme()).unwrap();
                if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
                    shot.matches_image(
                        std::path::Path::new(&directory)
                            .join(format!("panel-{shown:?}-{}", size.width)),
                    )
                    .unwrap();
                }
            }
        }
    }

    #[test]
    fn a_lane_dragged_among_the_shown_lanes_moves_the_channel_among_all_of_them() {
        let _turn = render_turn();
        let mut app = app_with_capture("lane-move");
        // Four lanes of the first four channels; all 32 channels are in the order.
        assert_eq!(app.presentation_move(0, 3), Some((0, 3)));
        assert_eq!(app.presentation_move(2, 2), None);
        assert_eq!(app.presentation_move(0, 9), None);
        // Channel 3 goes to the top of the order: the lanes are now 3, 0, 1, 2.
        let _ = app.update(Message::Run(Command::MoveChannel { from: 3, to: 0 }));
        assert_eq!(app.presentation_move(0, 3), Some((0, 3)));
        assert_eq!(app.presentation_move(3, 0), Some((3, 0)));
        let _ = app.update(Message::MoveLane { from: 3, to: 0 });
        let order: Vec<u8> = app
            .shell
            .state()
            .presentation
            .channels
            .iter()
            .take(4)
            .map(|c| c.index)
            .collect();
        assert_eq!(order, [2, 3, 0, 1]);
    }

    #[test]
    fn the_name_editor_accepts_with_enter_and_cancels_with_escape() {
        let _turn = render_turn();
        let mut app = app_with_capture("legend-name");
        let _ = app.update(Message::EditChannelName(2, 40.0));
        let _ = app.update(Message::ChannelNameInput("Clock".to_owned()));
        let _ = app.update(Message::ChannelNameAccept);
        assert!(app.editing.is_none());
        let name = |app: &App| {
            app.shell
                .state()
                .presentation
                .channels
                .iter()
                .find(|c| c.index == 2)
                .unwrap()
                .name
                .clone()
        };
        assert_eq!(name(&app), "Clock");
        // Cancelling keeps the old name.
        let _ = app.update(Message::EditChannelName(2, 40.0));
        let _ = app.update(Message::ChannelNameInput("Other".to_owned()));
        let _ = app.update(Message::ChannelNameCancel);
        assert_eq!(name(&app), "Clock");
        // An empty text gives back the default name (the host decides).
        let _ = app.update(Message::EditChannelName(2, 40.0));
        let _ = app.update(Message::ChannelNameInput(String::new()));
        let _ = app.update(Message::ChannelNameAccept);
        assert_eq!(name(&app), "");
    }

    /// Adds the sample decoder to the capture on screen, gives it its channels and waits for it.
    fn decode(app: &mut App) {
        let _ = app.update(Message::AddDecoderPicked(Choice {
            value: la_decoders::decoders::clocked_bits::ID.to_owned(),
            label: String::new(),
        }));
        let id = app.shell.state().decoders.instances[0].id;
        for (role, channel) in [("clock", 0_u8), ("data", 1)] {
            let _ = app.update(Message::DecoderChannelPicked(
                id,
                role.to_owned(),
                Choice {
                    value: Some(channel),
                    label: String::new(),
                },
            ));
        }
        let deadline = Instant::now().checked_add(Duration::from_secs(30)).unwrap();
        while app.shell.state().decoders.instances[0].state
            != la_host::decoders::InstanceState::Finished
        {
            let _ = app.update(Message::Tick(Instant::now()));
            assert!(Instant::now() < deadline, "the decoder did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_view_renders_decoder_lanes_and_overlays_at_two_zoom_levels() {
        let _turn = render_turn();
        let mut app = app_with_capture("decoder-render");
        decode(&mut app);
        for (name, start, end) in [
            ("decoder-zoomed", 100_000, 100_500),
            ("decoder-wide", 0, 300_000),
        ] {
            let _ = app.update(Message::Run(Command::ZoomToRange { start, end }));
            assert_eq!(app.plan.lanes.len(), 1, "{name}");
            assert!(app.plan.lanes[0].plan.represented() > 0, "{name}");
            assert!(!app.plan.overlays.is_empty(), "{name}");
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(1180.0, 720.0),
                app.view(),
            );
            let snapshot = ui.snapshot(&app.theme()).unwrap();
            if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
                snapshot
                    .matches_image(std::path::Path::new(&directory).join(name))
                    .unwrap();
            }
        }
    }

    static ECHO: la_decoders::descriptor::Descriptor = la_decoders::descriptor::Descriptor {
        id: "test.echo",
        ..la_decoders::decoders::clocked_bits::DESCRIPTOR
    };

    #[test]
    fn a_registered_test_decoder_appears_without_changing_the_shell() {
        let _turn = render_turn();
        let mut app = app_with_capture("decoder-registered");
        assert_eq!(app.shell.state().decoders.catalogue.len(), 2);
        app.shell
            .register_decoder(&ECHO, la_decoders::decoders::clocked_bits::create)
            .unwrap();
        let catalogue = &app.shell.state().decoders.catalogue;
        assert_eq!(catalogue.len(), 3);
        let echo = catalogue.iter().find(|c| c.id == "test.echo").unwrap();
        // The setup form is built from the descriptor: its signals and options.
        assert_eq!(echo.signals.len(), 2);
        assert_eq!(echo.options.len(), 4);
        // With two decoders the panel offers a list instead of a button; adding works from it.
        let _ = app.update(Message::AddDecoderPicked(Choice {
            value: "test.echo".to_owned(),
            label: String::new(),
        }));
        assert_eq!(app.shell.state().decoders.instances.len(), 1);
        let _ = app.update(Message::AddDecoderMenu);
        assert_eq!(
            app.shell.state().decoders.instances.len(),
            1,
            "the menu cannot choose for you"
        );
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        ui.snapshot(&app.theme()).unwrap();
    }

    #[test]
    fn the_view_renders_a_zoomed_capture_with_single_edges() {
        let _turn = render_turn();
        let mut app = app_with_capture("zoomed");
        let _ = app.update(Message::Run(Command::ZoomToRange {
            start: 150_000,
            end: 150_400,
        }));
        let _ = app.update(Message::Run(Command::RenameChannel {
            channel: 1,
            name: "SDA".into(),
        }));
        assert!(app.plan.channels.iter().any(|c| {
            c.segments
                .iter()
                .any(|s| matches!(s, la_host::plan::Segment::Edge { .. }))
        }));
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        let snapshot = ui.snapshot(&app.theme()).unwrap();
        if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
            snapshot
                .matches_image(std::path::Path::new(&directory).join("zoomed"))
                .unwrap();
        }
    }

    #[test]
    fn a_damaged_settings_file_shows_a_localized_message_with_its_buttons() {
        let _turn = render_turn();
        let dir = std::env::temp_dir().join(format!("la-app-banner-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("settings.json"), b"{ not json").unwrap();
        let mut config = ShellConfig::for_tests(&dir);
        config.log_location = Some(dir.join("logs"));
        let app = App::with_shell(Shell::new(config));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(app.shell.state().notices.len(), 1);
        assert!(app.banner().is_some());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        // The text of the message, the log button and the dismiss button are on screen.
        let shown = app
            .shell
            .notice_text(app.shell.state().notices.iter().next().unwrap());
        assert!(ui.find(shown.as_str()).is_ok());
        assert!(ui.find("Show log location").is_ok());
        assert!(ui.find("Dismiss").is_ok());
        let snapshot = ui.snapshot(&app.theme()).unwrap();
        if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
            snapshot
                .matches_image(std::path::Path::new(&directory).join("banner"))
                .unwrap();
        }
    }

    #[test]
    fn two_markers_show_their_distance_and_frequency() {
        let _turn = render_turn();
        let mut app = app_with_capture("markers");
        let _ = app.update(Message::Run(Command::SetSnap(false)));
        let _ = app.update(Message::Run(Command::ZoomToRange {
            start: 150_000,
            end: 150_400,
        }));
        for position in [150_080, 150_280] {
            let _ = app.update(Message::Run(Command::PlaceMarker {
                position,
                channel: 0,
            }));
        }
        let _ = app.update(Message::AreaResized(1000.0));
        // The measurement is in the Markers view of the side panel.
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Markers));
        assert_eq!(app.plan.markers.len(), 2);
        let text = app.shell.measurement_text().unwrap();
        assert!(text.distance.starts_with("200"), "{}", text.distance);
        assert!(text.frequency.is_some());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        let line = format!("Distance: {}", text.distance);
        assert!(ui.find(line.as_str()).is_ok());
        let snapshot = ui.snapshot(&app.theme()).unwrap();
        if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
            snapshot
                .matches_image(std::path::Path::new(&directory).join("markers"))
                .unwrap();
        }
    }

    #[test]
    fn the_title_names_the_file_and_marks_unsaved_changes() {
        let mut app = app_with_capture("title");
        assert!(
            app.title().starts_with("Untitled capture*"),
            "{}",
            app.title()
        );
        let dir = std::env::temp_dir().join(format!("la-app-title-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("my-capture.lasession");
        let _ = app.update(Message::Run(Command::SaveSessionAs(path.clone())));
        let deadline = Instant::now().checked_add(Duration::from_secs(20)).unwrap();
        while app.shell.operation_running() {
            let _ = app.update(Message::Tick(Instant::now()));
            assert!(Instant::now() < deadline, "the save did not end");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(path.exists());
        assert!(
            app.title().starts_with("my-capture.lasession \u{2014}"),
            "{}",
            app.title()
        );
        let _ = app.update(Message::Run(Command::RenameChannel {
            channel: 0,
            name: "x".into(),
        }));
        assert!(
            app.title().starts_with("my-capture.lasession* "),
            "{}",
            app.title()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closing_with_unsaved_data_shows_the_question_with_its_three_answers() {
        let _turn = render_turn();
        let mut app = app_with_capture("close");
        let _ = app.update(Message::CloseRequested);
        assert!(app.shell.state().pending_decision.is_some());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        for label in ["Unsaved capture", "Save", "Discard", "Cancel"] {
            assert!(ui.find(label).is_ok(), "{label}");
        }
        let snapshot = ui.snapshot(&app.theme()).unwrap();
        if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
            snapshot
                .matches_image(std::path::Path::new(&directory).join("unsaved"))
                .unwrap();
        }
    }

    #[test]
    fn a_running_save_shows_its_progress_with_a_cancel_button() {
        let _turn = render_turn();
        let app = app_with_capture("progress");
        let view = la_host::OperationView {
            kind: la_host::OperationKind::Save,
            done_bytes: 300,
            total_bytes: 1_000,
        };
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(600.0, 300.0),
            app.progress_dialog(view),
        );
        assert!(ui.find("Saving the session\u{2026}").is_ok());
        assert!(ui.find("Cancel").is_ok());
        let snapshot = ui.snapshot(&app.theme()).unwrap();
        if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
            snapshot
                .matches_image(std::path::Path::new(&directory).join("progress"))
                .unwrap();
        }
    }

    #[test]
    fn the_file_menu_lists_open_save_and_save_as_and_enables_them_by_state() {
        let app = app_with_capture("filemenu");
        let enabled = |key: &str| {
            app.table
                .iter()
                .find(|e| e.menu[1] == key)
                .map(|e| e.enabled_when.is_enabled(app.shell.state()))
        };
        assert_eq!(enabled("menu-file-open"), Some(true));
        assert_eq!(enabled("menu-file-save"), Some(true));
        assert_eq!(enabled("menu-file-save-as"), Some(true));
        let fresh = App::with_shell(Shell::new(ShellConfig::for_tests(&std::env::temp_dir())));
        let enabled = |key: &str| {
            fresh
                .table
                .iter()
                .find(|e| e.menu[1] == key)
                .map(|e| e.enabled_when.is_enabled(fresh.shell.state()))
        };
        assert_eq!(enabled("menu-file-save"), Some(false), "nothing to save");
        assert_eq!(enabled("menu-file-open"), Some(true));
    }

    #[test]
    fn every_screen_renders_in_every_language_at_the_minimum_window_size() {
        let _turn = render_turn();
        let mut app = app_with_capture("languages");
        let _ = app.update(Message::Run(Command::SetSnap(false)));
        let _ = app.update(Message::Run(Command::PlaceMarker {
            position: 100_000,
            channel: 0,
        }));
        let _ = app.update(Message::Run(Command::TestError("session.damaged")));
        // A decoder with its lane and overlays is on screen in every language.
        decode(&mut app);
        for language in la_i18n::Language::ALL {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            assert_eq!(app.shell.state().language, language);
            let size = iced::Size::new(800.0, 560.0);
            let menu = app.t("menu-file");
            let settings_title = app.t("settings-title");
            let about_title = app.t("about-title");
            // The main window, the settings dialog and the about dialog.
            let mut ui =
                iced_test::Simulator::with_size(crate::fonts::settings(), size, app.view());
            assert!(ui.find(menu.as_str()).is_ok(), "{language:?}: {menu}");
            let main_shot = ui.snapshot(&app.theme()).unwrap();
            if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
                main_shot
                    .matches_image(
                        std::path::Path::new(&directory).join(format!("main-{}", language.tag())),
                    )
                    .unwrap();
            }
            // The Decoders panel at the smallest window: its labels are there and it renders.
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                iced::Size::new(280.0, 400.0),
                app.decoders_panel(),
            );
            for key in [
                "panel-decoders",
                "decoder-lane-show",
                "decoder-run",
                "decoder-state-finished",
            ] {
                let label = app.t(key);
                assert!(
                    ui.find(label.as_str()).is_ok() || key == "decoder-state-finished",
                    "{language:?} {key}: {label}"
                );
            }
            let _ = ui.snapshot(&app.theme()).unwrap();
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                size,
                app.settings_dialog(),
            );
            assert!(ui.find(settings_title.as_str()).is_ok(), "{language:?}");
            for key in [
                "settings-language",
                "settings-theme",
                "settings-scale",
                "settings-memory",
            ] {
                let label = app.t(key);
                assert!(
                    ui.find(label.as_str()).is_ok(),
                    "{language:?} {key}: {label}"
                );
            }
            let shot = ui.snapshot(&app.theme()).unwrap();
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                size,
                app.about_dialog(),
            );
            assert!(ui.find(about_title.as_str()).is_ok(), "{language:?}");
            let _ = ui.snapshot(&app.theme()).unwrap();
            if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
                shot.matches_image(
                    std::path::Path::new(&directory).join(format!("settings-{}", language.tag())),
                )
                .unwrap();
            }
        }
    }

    #[test]
    fn the_interface_scale_steps_and_follows_the_setting() {
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(
            &std::env::temp_dir().join(format!("la-app-scale-{}", std::process::id())),
        )));
        assert!((app.scale_factor() - 1.0).abs() < f32::EPSILON);
        let _ = app.update(Message::ScaleStep(1));
        assert!((app.scale_factor() - 1.25).abs() < f32::EPSILON);
        for _ in 0..10 {
            let _ = app.update(Message::ScaleStep(1));
        }
        assert!(
            (app.scale_factor() - 2.0).abs() < f32::EPSILON,
            "the largest step is 200 %"
        );
        for _ in 0..10 {
            let _ = app.update(Message::ScaleStep(-1));
        }
        assert!(
            (app.scale_factor() - 0.75).abs() < f32::EPSILON,
            "the smallest step is 75 %"
        );
    }

    #[test]
    fn ctrl_equals_and_ctrl_minus_change_the_scale_and_plain_plus_still_zooms() {
        let table = table();
        let ctrl = Modifiers::CTRL;
        let up = entry_for(&table, &Key::Character("=".into()), ctrl).unwrap();
        assert_eq!(up.action, Action::UiScaleUp);
        // Ctrl+Shift+= is Ctrl and the plus sign on many layouts.
        let up = entry_for(&table, &Key::Character("+".into()), ctrl | Modifiers::SHIFT).unwrap();
        assert_eq!(up.action, Action::UiScaleUp);
        let down = entry_for(&table, &Key::Character("-".into()), ctrl).unwrap();
        assert_eq!(down.action, Action::UiScaleDown);
        let zoom = entry_for(&table, &Key::Character("=".into()), Modifiers::empty()).unwrap();
        assert_eq!(
            zoom.action,
            Action::Command(Command::ZoomStep(la_host::ZoomDirection::In))
        );
    }

    #[test]
    fn a_memory_limit_typed_in_the_settings_applies_only_when_valid() {
        let dir = std::env::temp_dir().join(format!("la-app-memory-{}", std::process::id()));
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        let before = app.shell.state().settings.memory_limit_bytes;
        let _ = app.update(Message::MemoryText("abc".into()));
        let _ = app.update(Message::MemoryText("0".into()));
        assert_eq!(app.shell.state().settings.memory_limit_bytes, before);
        let _ = app.update(Message::MemoryText("512".into()));
        assert_eq!(app.shell.state().settings.memory_limit_bytes, 512 << 20);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Responsiveness on a capture of 100 million samples over 16 channels (spec SC-002, SC-004,
    /// SC-009). Run in release mode:
    /// `cargo test -p la-app --release -- --ignored measure --nocapture`.
    ///
    /// It measures the work the interface thread does for each action (the update, the plan, the
    /// widget tree) and, as an upper bound for a frame, a full render to a picture that includes
    /// creating a software renderer. The time the graphics card needs is not in these numbers.
    #[test]
    #[ignore = "a measurement, not a check; run it on purpose"]
    fn measure_responsiveness_on_a_hundred_million_samples() {
        use std::time::Duration;

        fn pct(mut v: Vec<Duration>, p: usize) -> Duration {
            v.sort_unstable();
            v.get((v.len().saturating_sub(1)) * p / 100)
                .copied()
                .unwrap_or_default()
        }
        fn row(label: &str, v: &[Duration]) {
            println!(
                "{label:<34} n={:<4} p50 {:>7.2} ms  p95 {:>7.2} ms  max {:>7.2} ms",
                v.len(),
                pct(v.to_vec(), 50).as_secs_f64() * 1e3,
                pct(v.to_vec(), 95).as_secs_f64() * 1e3,
                pct(v.to_vec(), 100).as_secs_f64() * 1e3
            );
        }

        let _turn = render_turn();
        let dir = std::env::temp_dir().join(format!("la-app-measure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        for edit in [
            ConfigEdit::Realtime(false),
            ConfigEdit::Channels(0xffff),
            ConfigEdit::SampleRate(100_000_000),
            ConfigEdit::Length(100_000_000),
            ConfigEdit::Seed(Some(5)),
            ConfigEdit::AcceptBudgetRisk(true),
        ] {
            let _ = app.update(Message::Run(Command::EditConfig(edit)));
        }
        let _ = app.update(Message::AreaResized(1900.0));

        // A capture: the longest time the interface thread is busy with one call.
        let mut busy = Vec::new();
        let started = Instant::now();
        let _ = app.update(Message::Run(Command::StartCapture));
        while app.shell.state().activity != Activity::Idle {
            let t = Instant::now();
            let _ = app.update(Message::Tick(Instant::now()));
            let _ = app.view();
            busy.push(t.elapsed());
            std::thread::sleep(Duration::from_millis(20));
        }
        println!(
            "capture of {} samples took {:.1} s",
            app.shell
                .state()
                .capture
                .as_ref()
                .unwrap()
                .snapshot
                .length(),
            started.elapsed().as_secs_f64()
        );
        row("during the capture (tick + view)", &busy);

        // Zoom and pan: the update (with the frame plan) and the widget tree.
        let mut update = Vec::new();
        let mut views = Vec::new();
        for i in 0..400 {
            let message = match i % 4 {
                0 => Message::Run(Command::Zoom {
                    factor: la_host::ZoomFactor {
                        numerator: 4,
                        denominator: 5,
                    },
                    anchor: la_host::ZoomAnchor::Column(900),
                }),
                1 => Message::Run(Command::Pan { pixels: 120 }),
                2 => Message::Run(Command::Pan { pixels: -300 }),
                _ => Message::Run(Command::Zoom {
                    factor: la_host::ZoomFactor {
                        numerator: 5,
                        denominator: 4,
                    },
                    anchor: la_host::ZoomAnchor::Column(400),
                }),
            };
            let t = Instant::now();
            let _ = app.update(message);
            update.push(t.elapsed());
            let t = Instant::now();
            let _ = app.view();
            views.push(t.elapsed());
        }
        row("zoom and pan: update + plan", &update);
        row("zoom and pan: widget tree", &views);

        // A frame drawn to a picture: the renderer setup apart from the drawing and the readback
        // of the picture (the readback is not part of a frame on screen).
        let mut setups = Vec::new();
        let mut frames = Vec::new();
        for _ in 0..6 {
            let _ = app.update(Message::Run(Command::Pan { pixels: 200 }));
            let t = Instant::now();
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(1900.0, 720.0),
                app.view(),
            );
            setups.push(t.elapsed());
            let t = Instant::now();
            let _ = ui.snapshot(&app.theme()).unwrap();
            frames.push(t.elapsed());
        }
        row("renderer setup (once per window)", &setups);
        row("draw + picture readback", &frames);

        // Save and open: the interface thread while the background operation runs.
        let path = dir.join("big.lasession");
        let mut worst_busy = Duration::ZERO;
        for (label, command) in [
            ("save", Command::SaveSessionAs(path.clone())),
            ("open", Command::OpenSession(path.clone())),
        ] {
            let started = Instant::now();
            let _ = app.update(Message::Run(command));
            let mut ticks = Vec::new();
            while app.shell.operation_running() {
                let t = Instant::now();
                let _ = app.update(Message::Tick(Instant::now()));
                let _ = app.view();
                ticks.push(t.elapsed());
                std::thread::sleep(Duration::from_millis(20));
            }
            println!("{label} took {:.2} s", started.elapsed().as_secs_f64());
            row(&format!("during {label} (tick + view)"), &ticks);
            worst_busy = worst_busy.max(pct(ticks, 100));
        }
        assert!(
            app.shell.state().notices.is_empty(),
            "a failure was reported during the measurement"
        );

        // Changing the language: the update and a full repaint.
        let mut language = Vec::new();
        let mut language_total = Vec::new();
        for l in la_i18n::Language::ALL {
            let t = Instant::now();
            let _ = app.update(Message::Run(Command::SetLanguage(l)));
            let view = app.view();
            let done_view = t.elapsed();
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(1900.0, 720.0),
                view,
            );
            let t2 = Instant::now();
            let _ = ui.snapshot(&app.theme()).unwrap();
            language.push(done_view + t2.elapsed());
            language_total.push(done_view);
        }
        row("language change: update + widgets", &language_total);
        row("language change + draw (no setup)", &language);
        println!(
            "worst single call of the interface thread during save and open: {:.2} ms",
            worst_busy.as_secs_f64() * 1e3
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// How long the work before the first window takes. Run in release mode:
    /// `cargo test -p la-app --release -- --ignored startup --nocapture`.
    #[test]
    #[ignore = "a measurement, not a check; run it on purpose"]
    fn measure_startup() {
        let dir = std::env::temp_dir().join(format!("la-app-startup-{}", std::process::id()));
        for round in 0..3 {
            let t = Instant::now();
            let shell = Shell::new(ShellConfig::for_user());
            let shell_time = t.elapsed();
            let t = Instant::now();
            let app = App::with_shell(shell);
            let app_time = t.elapsed();
            let _ = &dir;
            println!(
                "round {round}: Shell::new {:.1} ms, App::with_shell {:.1} ms ({} devices)",
                shell_time.as_secs_f64() * 1e3,
                app_time.as_secs_f64() * 1e3,
                app.shell.state().devices.len()
            );
        }
    }

    #[test]
    fn channels_that_are_not_in_a_row_can_be_chosen_for_recording_in_the_panel() {
        let _turn = render_turn();
        let dir = std::env::temp_dir().join(format!("la-app-record-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        for edit in [
            ConfigEdit::Realtime(false),
            ConfigEdit::SampleRate(1_000_000),
            ConfigEdit::Length(20_000),
            ConfigEdit::Seed(Some(8)),
        ] {
            let _ = app.update(Message::Run(Command::EditConfig(edit)));
        }
        // Names and colours do not wait for a capture.
        let _ = app.update(Message::ChannelName(4, "SDA".into()));
        let _ = app.update(Message::ChannelColour(4));
        {
            let style = app
                .shell
                .state()
                .presentation
                .channels
                .iter()
                .find(|c| c.index == 4)
                .unwrap();
            assert_eq!(style.name, "SDA");
            assert_eq!(style.colour, Colour::Palette(5));
        }
        // Start from nothing and tick channels 0, 2 and 5, as a person would.
        for channel in 0..32 {
            let _ = app.update(Message::ChannelVisible(channel, false));
        }
        assert_eq!(app.shell.state().config.enabled_channels, 0);
        assert!(
            !app.shell.state().config_problems.is_empty(),
            "no channel at all keeps Start unavailable (without a message: the flags show it)"
        );
        for channel in [0, 2, 5] {
            let _ = app.update(Message::ChannelVisible(channel, true));
        }
        assert_eq!(app.shell.state().config.enabled_channels, 0b100101);
        assert!(app.shell.state().config_problems.is_empty());

        // The panel before a capture lists the channels with their flags.
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        assert!(ui.find("Channel 31").is_ok());
        let snapshot = ui.snapshot(&app.theme()).unwrap();
        if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
            snapshot
                .matches_image(std::path::Path::new(&directory).join("record-panel"))
                .unwrap();
        }
        drop(ui);

        // Recording gives exactly those channels, in the capture and in the panel after it.
        let _ = app.update(Message::Run(Command::StartCapture));
        let deadline = Instant::now().checked_add(Duration::from_secs(20)).unwrap();
        while app.shell.state().activity != Activity::Idle {
            let _ = app.update(Message::Tick(Instant::now()));
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(5));
        }
        let shown: Vec<u8> = app.plan.channels.iter().map(|c| c.channel).collect();
        assert_eq!(shown, [0, 2, 5]);

        // With the capture on screen the same flags still choose what the next capture records,
        // and names and colours can be changed whether or not there is a capture.
        let _ = app.update(Message::ChannelVisible(7, true));
        assert_eq!(app.shell.state().config.enabled_channels, 0b1010_0101);
        let still: Vec<u8> = app.plan.channels.iter().map(|c| c.channel).collect();
        assert_eq!(
            still,
            [0, 2, 5],
            "channel 7 has no data in this capture, so no lane appears for it yet"
        );
        let _ = app.update(Message::ChannelName(2, "SCL".into()));
        assert_eq!(
            app.shell
                .state()
                .presentation
                .channels
                .iter()
                .find(|c| c.index == 2)
                .unwrap()
                .name,
            "SCL"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn accelerators_are_described_for_the_menu() {
        assert_eq!(describe(Accelerator::ctrl_shift("S")), "Ctrl+Shift+S");
        assert_eq!(describe(Accelerator::key("F5")), "F5");
    }
}
