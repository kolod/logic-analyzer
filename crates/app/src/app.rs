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
use la_host::shortcuts::{Accelerator, Action, ShortcutEntry};
use la_host::window::ViewSize;
use la_host::{
    Activity, Changes, Command, ConfigEdit, Decision, Outcome, Shell, ShellConfig, SignalProfile,
    Tick, Unsaved,
};
use la_i18n::{Args, Key};

use crate::icons::{self, Icon};
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
    /// A sample rate was chosen from the list of the device.
    RatePicked(RateChoice),
    /// The capture length text changed.
    LengthText(String),
    /// The seed text changed.
    SeedText(String),
    /// A signal profile was chosen.
    ProfilePicked(ProfileChoice),
    /// The system theme changed.
    SystemTheme(theme::Mode),
    /// The window asked to close.
    CloseRequested,
    /// A channel was shown or hidden.
    ChannelVisible(u8, bool),
    /// All the channels (`true`) or none (`false`) are recorded.
    ChannelsAll(bool),
    /// A press on the splitter between the signals and the panel.
    SplitterPressed,
    /// The pointer moved, at this x in the window, while the splitter is held.
    SplitterMoved(f32),
    /// The button was released: the width is kept.
    SplitterReleased,
    /// The box of the question about an unsaved capture.
    RememberToggled(bool),
    /// An answer to the question about an unsaved capture, remembered if the box is ticked.
    UnsavedAnswer(Decision),
    /// The box of the settings: remember an answer (`true`) or ask every time.
    RememberInSettings(bool),
    /// The remembered answer chosen in the settings.
    RememberedAnswer(Decision),
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
    /// A marker shortcut or menu entry: previous, next, remove selected, snap.
    MarkerAction(MarkerAction),
    /// Where the pointer is over the signal area while a marker is being placed (`None`: it is
    /// not over it).
    PlacementPointer(Option<la_host::markers::Pointer>, Instant),
    /// A frame of the clock of the automatic zoom (only while a marker is placed or dragged).
    DwellTick(Instant),
    /// The actions of a marker are opened or closed (its swatch or its menu button).
    MarkerMenu(u32),
    /// A marker was pressed with the right button in the signal view: show its actions.
    MarkerContext(u32),
    /// The comment of a marker is being edited in its action list.
    MarkerComment(u32, String),
    /// A single marker is checked or unchecked for making a pair.
    MarkerCheck(u32, bool),
    /// The menu entry that adds a decoder.
    /// An entry of the open menu that has a submenu (the index in the table) is pressed.
    SubmenuToggled(usize),
    /// "Remove unavailable entries" of the recent files.
    RemoveMissingRecent,
    /// A source was chosen for a decoder instance (the capture, or another instance).
    DecoderSourcePicked(u32, Choice<Source>),
    /// A decoder was chosen in the list of decoders to add.
    AddDecoderPicked(Choice<String>),
    /// A decoder instance was renamed in the panel.
    /// The properties of a decoder are folded or unfolded.
    DecoderToggled(u32),
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

/// A sample rate in the chooser of a device that has a list of rates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateChoice {
    hz: u64,
}

/// A rate as a person reads it: whole megahertz, whole kilohertz, else hertz.
fn format_rate(hz: u64) -> String {
    let (unit, divisor) = if hz >= 1_000_000 && hz.checked_rem(1_000_000) == Some(0) {
        ("MHz", 1_000_000)
    } else if hz >= 1_000 && hz.checked_rem(1_000) == Some(0) {
        ("kHz", 1_000)
    } else {
        ("Hz", 1)
    };
    format!("{} {unit}", hz.checked_div(divisor).unwrap_or(hz))
}

impl fmt::Display for RateChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&format_rate(self.hz))
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

/// The number of columns of a text in a monospaced font (a wide character counts twice).
fn columns(text: &str) -> usize {
    text.chars()
        .map(|c| if c.len_utf8() >= 3 { 2 } else { 1 })
        .sum()
}

/// Pads the changing parts of the status line to the widest each has been (`widths` remembers
/// them), with no-break spaces that a layout keeps, so the line does not shift while the digits
/// change. A part that is missing stays missing.
fn steady_parts(widths: &mut [usize; 3], parts: [Option<String>; 3]) -> [Option<String>; 3] {
    let mut out = [None, None, None];
    for (index, part) in parts.into_iter().enumerate() {
        let (Some(text), Some(width), Some(slot)) =
            (part, widths.get_mut(index), out.get_mut(index))
        else {
            continue;
        };
        *width = (*width).max(columns(&text));
        let pad = width.saturating_sub(columns(&text));
        *slot = Some(format!("{text}{}", "\u{a0}".repeat(pad)));
    }
    out
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
    /// The entry of the open menu (its index in the table) whose submenu is open.
    open_submenu: Option<usize>,
    rate_text: String,
    /// The widest the elapsed time, the samples and the memory of the status line have been in
    /// this capture, in columns: they are padded to it so the line does not shift.
    status_widths: std::cell::Cell<[usize; 3]>,
    length_text: String,
    length_error: Option<la_host::config::SecondsError>,
    seed_text: String,
    /// What the pointer means to the marker being placed (the host's answer to the pointer).
    placement_preview: Option<la_host::markers::Resolved>,
    /// The marker whose actions are open in the Markers view.
    marker_menu: Option<u32>,
    /// The comments of the markers as their text fields hold them.
    marker_comments: std::collections::BTreeMap<u32, String>,
    /// The single markers that are checked, to be made a pair.
    marker_checked: std::collections::BTreeSet<u32>,
    /// How long the pointer has rested while a marker is placed or dragged.
    dwell: crate::dwell::Dwell,
    /// The animation of an automatic zoom that is running.
    zoom_animation: Option<crate::dwell::ZoomAnimation>,
    /// The decoders whose properties are folded away.
    folded_decoders: std::collections::BTreeSet<u32>,
    option_texts: std::collections::BTreeMap<(u32, String), String>,
    messages_open: bool,
    settings_open: bool,
    /// The box "Remember my choice" of the question about an unsaved capture is ticked.
    remember_choice: bool,
    /// The splitter of the panel is being dragged.
    splitter: Option<SplitterDrag>,
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

/// The room around the content of every view of the panel: the same on all four sides, so the
/// left and the right margins look alike.
const PANEL_PADDING: iced::Padding = iced::Padding {
    top: 8.0,
    right: 8.0,
    bottom: 8.0,
    left: 8.0,
};

/// The drivers for real hardware the application registers at start-up. The host knows only the
/// device abstraction; this is the one place that lists the drivers.
pub(crate) fn device_providers() -> Vec<std::sync::Arc<dyn la_devices::DeviceProvider>> {
    vec![la_driver_weact::provider()]
}

/// The height of a channel button and the gap between two; the buttons share the width of the panel.
const CHANNEL_BUTTON: f32 = 28.0;
const CHANNEL_GAP: f32 = 3.0;

/// A drag of the splitter between the signals and the panel.
#[derive(Debug, Clone, Copy)]
struct SplitterDrag {
    /// Where the pointer was when the first move came; the width follows the distance from it.
    origin: Option<f32>,
    /// The width now, while it is dragged.
    width: f32,
    /// The width when the drag began.
    start_width: f32,
}

/// The index `SubmenuToggled` carries for the recent files of the File menu (not a table entry).
const RECENT_SUBMENU: usize = usize::MAX;

/// How many entries of the menu `title` are in the table: the row where the extra entry follows.
fn table_count_in(table: &[ShortcutEntry], title: &str) -> usize {
    table.iter().filter(|e| e.menu[0] == title).count()
}

impl App {
    /// Builds the application with the host shell of the current user.
    #[must_use]
    pub fn new() -> Self {
        let shell = Shell::new(ShellConfig::for_user());
        for provider in device_providers() {
            shell.add_provider(provider);
        }
        Self::with_shell(shell)
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
            open_submenu: None,
            rate_text: String::new(),
            status_widths: std::cell::Cell::new([0; 3]),
            length_text: String::new(),
            length_error: None,
            seed_text: String::new(),
            placement_preview: None,
            marker_menu: None,
            marker_comments: std::collections::BTreeMap::new(),
            marker_checked: std::collections::BTreeSet::new(),
            dwell: crate::dwell::Dwell::default(),
            zoom_animation: None,
            folded_decoders: std::collections::BTreeSet::new(),
            option_texts: std::collections::BTreeMap::new(),
            messages_open: false,
            settings_open: false,
            remember_choice: false,
            splitter: None,
            about_open: false,
            memory_text: String::new(),
            decoder_memory_text: String::new(),
            window_rect: [0, 0, 0, 0],
            editing: None,
        };
        app.sync_texts();
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
        // What the user typed stays while it still means the length in the configuration.
        let (rate, length) = (config.sample_rate_hz, config.length);
        if la_host::config::samples_for_seconds(rate, &self.length_text) != Ok(length) {
            self.length_text = la_host::config::seconds_for_samples(rate, length);
            self.length_error = None;
        }
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
        let remembered = settings
            .unsaved_choice
            .as_deref()
            .and_then(Decision::from_setting);
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
            text(self.t("settings-unsaved-title")).size(14),
            checkbox(remembered.is_some())
                .label(self.t("settings-remember-choice"))
                .on_toggle(Message::RememberInSettings),
        ]
        .spacing(10)
        .max_width(460);
        if remembered.is_some() {
            // The two answers that can be remembered, side by side; the remembered one is filled.
            let mut answers = row![].spacing(8);
            for (decision, key) in [
                (Decision::Save, "dialog-save"),
                (Decision::Discard, "dialog-discard"),
            ] {
                answers = answers.push(
                    button(text(self.t(key)))
                        .style(if remembered == Some(decision) {
                            button::primary
                        } else {
                            button::secondary
                        })
                        .on_press(Message::RememberedAnswer(decision)),
                );
            }
            body = body.push(answers);
        }
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
            MarkerAction::Previous | MarkerAction::Next => {
                let forward = action == MarkerAction::Next;
                match state.markers.jump_target(centre, forward) {
                    Some(id) => self.run(Command::ScrollToMarker(id)),
                    None => Task::none(),
                }
            }
            MarkerAction::RemoveSelected => match state.markers.selected {
                Some(id) => self.run(Command::RemoveMarker(id)),
                None => Task::none(),
            },
            MarkerAction::ToggleSnap => {
                let snap = !state.snap;
                self.run(Command::SetSnap(snap))
            }
        }
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
        // While the splitter is held, the pointer is followed over the whole window.
        let splitter = if self.splitter.is_some() {
            iced::event::listen_with(|event, _status, _window| match event {
                iced::Event::Mouse(iced::mouse::Event::CursorMoved { position }) => {
                    Some(Message::SplitterMoved(position.x))
                }
                iced::Event::Mouse(iced::mouse::Event::ButtonReleased(
                    iced::mouse::Button::Left,
                )) => Some(Message::SplitterReleased),
                _ => None,
            })
        } else {
            Subscription::none()
        };
        Subscription::batch([
            splitter,
            keyboard::listen().map(Message::Key),
            iced::time::every(period).map(Message::Tick),
            // The clock of the automatic zoom runs only while a marker is placed or dragged.
            if self.shell.state().placement.is_some() || self.shell.state().drag.is_some() {
                iced::time::every(Duration::from_millis(33)).map(Message::DwellTick)
            } else {
                Subscription::none()
            },
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

    /// Keeps what the Markers view holds in step with the markers: a menu, a check or a comment
    /// of a marker that is gone goes away, and the comment fields show the comments (except while
    /// one is being typed, which would move the cursor).
    fn sync_markers(&mut self, editing_comment: bool) {
        let state = self.shell.state();
        let exists = |id: &u32| state.markers.get(*id).is_some();
        if self.marker_menu.as_ref().is_some_and(|id| !exists(id)) {
            self.marker_menu = None;
        }
        self.marker_checked.retain(exists);
        if !editing_comment {
            self.marker_comments = crate::marker_view::comments_of(state);
        }
    }

    /// The texts of the Markers view.
    fn marker_strings(&self) -> crate::marker_view::Strings {
        crate::marker_view::Strings {
            title: self.t("panel-markers"),
            single: self.t("marker-single-add"),
            pair: self.t("marker-pair-add"),
            empty: self.t("marker-empty"),
            limit: self.t("marker-limit"),
            delete: self.t("marker-delete"),
            comment: self.t("marker-comment"),
            make_pair: self.t("marker-make-pair"),
            origin: self.t("marker-action-origin"),
            origin_reset: self.t("marker-action-origin-reset"),
            cut_left: self.t("marker-action-cut-left"),
            cut_right: self.t("marker-action-cut-right"),
            scroll: self.t("marker-action-scroll"),
            trim: self.t("marker-action-trim"),
            fit: self.t("marker-action-fit"),
            split: self.t("marker-action-split"),
            disabled_running: self.t("marker-cut-disabled-running"),
            disabled_nothing: self.t("marker-cut-disabled-nothing"),
            colours: [
                "marker-colour-0",
                "marker-colour-1",
                "marker-colour-2",
                "marker-colour-3",
                "marker-colour-4",
                "marker-colour-5",
                "marker-colour-6",
                "marker-colour-7",
            ]
            .iter()
            .map(|key| self.t(key))
            .collect(),
        }
    }

    /// One frame of the clock of the automatic zoom: starts the zoom when the pointer has rested
    /// over a crowded zone, and steps the animation of one that is running.
    fn dwell_tick(&mut self, now: Instant) {
        let state = self.shell.state();
        if state.placement.is_none() && state.drag.is_none() {
            self.dwell.reset();
            self.zoom_animation = None;
            return;
        }
        if let Some(animation) = self.zoom_animation {
            // The user took the view over (a wheel turn, a pan): the zoom stops.
            if state.snap_zoom.is_none() {
                self.zoom_animation = None;
                return;
            }
            let ((start, length), done) = animation.frame(now);
            let _ = self.run(Command::PlacementZoomTo { start, length });
            if done {
                self.zoom_animation = None;
            }
            return;
        }
        if let Some(pointer) = self.dwell.due(now)
            && let Some(target) = self.shell.crowded_zoom_target(pointer)
        {
            let shown = if state.auto_fit {
                (0, state.capture.as_ref().map_or(0, |c| c.snapshot.length()))
            } else {
                (state.window.start, state.window.length)
            };
            let animation = crate::dwell::ZoomAnimation::new(shown, target, now);
            let ((start, length), done) = animation.frame(now);
            let _ = self.run(Command::PlacementZoomTo { start, length });
            if !done {
                self.zoom_animation = Some(animation);
            }
        }
    }

    /// Runs a host command and turns its outcome into a task.
    fn run(&mut self, command: Command) -> Task<Message> {
        let editing_comment = matches!(command, Command::SetMarkerComment { .. });
        // A drag snaps like a placement: what the pointer resolves to is shown (the ring).
        let drag_preview = match &command {
            Command::MoveMarkerEnd { pointer, .. } => Some(self.shell.resolve_pointer(*pointer)),
            _ => None,
        };
        let outcome = self.shell.apply(command);
        if drag_preview.is_some() {
            self.placement_preview = drag_preview;
        }
        if self.shell.state().placement.is_none() && self.shell.state().drag.is_none() {
            self.placement_preview = None;
        }
        if self.shell.state().placement.is_none() && self.shell.state().drag.is_none() {
            self.dwell.reset();
            self.zoom_animation = None;
        }
        self.sync_markers(editing_comment);
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
        let stage = if matches!(message, Message::Tick(_)) {
            crate::frame_log::Stage::UpdateTick
        } else {
            crate::frame_log::Stage::UpdateOther
        };
        let started = Instant::now();
        let task = self.update_message(message);
        crate::frame_log::record(stage, started.elapsed());
        task
    }

    fn update_message(&mut self, message: Message) -> Task<Message> {
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
            }) if self.marker_menu.is_some() && self.shell.state().placement.is_none() => {
                self.marker_menu = None;
                Task::none()
            }
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
                    Some(Action::PreviousMarker) => {
                        self.update(Message::MarkerAction(MarkerAction::Previous))
                    }
                    Some(Action::NextMarker) => {
                        self.update(Message::MarkerAction(MarkerAction::Next))
                    }
                    Some(Action::RemoveSelectedMarker) => {
                        self.update(Message::MarkerAction(MarkerAction::RemoveSelected))
                    }
                    Some(Action::AddDecoder) => {
                        self.open_add_decoder_submenu();
                        Task::none()
                    }
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
                Some(edit) if edit.text != edit.original => self.run(Command::RenameChannel {
                    channel: edit.channel,
                    name: edit.text,
                }),
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
            Message::PlacementPointer(pointer, now) => {
                self.placement_preview = pointer.map(|p| self.shell.resolve_pointer(p));
                self.dwell.moved(pointer, now);
                Task::none()
            }
            Message::DwellTick(now) => {
                self.dwell_tick(now);
                Task::none()
            }
            Message::MarkerMenu(id) => {
                self.marker_menu = (self.marker_menu != Some(id)).then_some(id);
                Task::none()
            }
            Message::MarkerContext(id) => {
                self.marker_menu = Some(id);
                let _ = self.run(Command::SelectMarker(Some(id)));
                self.run(Command::SetSidePanel(la_host::SidePanel::Markers))
            }
            Message::MarkerComment(id, comment) => {
                let comment = crate::marker_view::limit_comment(&comment);
                self.marker_comments.insert(id, comment.clone());
                self.run(Command::SetMarkerComment { id, comment })
            }
            Message::MarkerCheck(id, on) => {
                if on {
                    self.marker_checked.insert(id);
                } else {
                    self.marker_checked.remove(&id);
                }
                Task::none()
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
            Message::SplitterPressed => {
                self.splitter = Some(SplitterDrag {
                    origin: None,
                    width: self.panel_width(),
                    start_width: self.panel_width(),
                });
                Task::none()
            }
            Message::SplitterMoved(x) => {
                if let Some(drag) = &mut self.splitter {
                    // The pointer moving left makes the panel wider.
                    let origin = *drag.origin.get_or_insert(x);
                    let (low, high) = la_host::settings::PANEL_WIDTH_RANGE;
                    drag.width = (drag.start_width + (origin - x)).clamp(low as f32, high as f32);
                }
                Task::none()
            }
            Message::SplitterReleased => match self.splitter.take() {
                Some(drag) => self.run(Command::SetPanelWidth(drag.width.round() as u32)),
                None => Task::none(),
            },
            Message::RememberToggled(on) => {
                self.remember_choice = on;
                Task::none()
            }
            Message::UnsavedAnswer(decision) => {
                // Only Save and Discard can be remembered; Cancel never is.
                let remember = std::mem::take(&mut self.remember_choice);
                if remember && decision != Decision::Cancel {
                    let _ = self.run(Command::SetUnsavedChoice(Some(decision)));
                }
                self.run(Command::Decide(decision))
            }
            Message::RememberInSettings(on) => {
                self.run(Command::SetUnsavedChoice(on.then_some(Decision::Save)))
            }
            Message::RememberedAnswer(decision) => {
                self.run(Command::SetUnsavedChoice(Some(decision)))
            }
            Message::ChannelsAll(on) => {
                // One edit of the recording mask: the flags of the channels follow it.
                let mask = if on {
                    self.shell
                        .state()
                        .presentation
                        .channels
                        .iter()
                        .filter(|c| c.index < 32)
                        .fold(0_u32, |mask, c| mask | 1_u32 << c.index)
                } else {
                    0
                };
                self.run(Command::EditConfig(ConfigEdit::Channels(mask)))
            }
            Message::ChannelVisible(channel, visible) => {
                self.run(Command::SetChannelVisible { channel, visible })
            }
            Message::AreaResized(width) => {
                let pixels = width.max(1.0) as u32;
                self.run(Command::SetViewSize(ViewSize { pixels, scale: 1.0 }))
            }
            Message::Run(command) => {
                self.open_menu = None;
                self.open_submenu = None;
                // A marker being dragged rests like one being placed.
                if let Command::MoveMarkerEnd { pointer, .. } = &command {
                    self.dwell.moved(Some(*pointer), Instant::now());
                }
                self.run(command)
            }
            Message::RemoveMissingRecent => {
                self.open_menu = None;
                self.open_submenu = None;
                let missing: Vec<_> = self
                    .shell
                    .recent_files()
                    .into_iter()
                    .filter(|f| !f.exists)
                    .map(|f| f.path)
                    .collect();
                let mut task = Task::none();
                for path in missing {
                    task = self.run(Command::RemoveRecentFile(path));
                }
                task
            }
            Message::SubmenuToggled(entry) => {
                self.open_submenu = if self.open_submenu == Some(entry) {
                    None
                } else {
                    Some(entry)
                };
                Task::none()
            }
            Message::DecoderSourcePicked(id, choice) => self.run(Command::StackDecoder {
                id,
                source: choice.value,
            }),
            Message::AddDecoderPicked(choice) => self.run(Command::AddDecoder {
                decoder: choice.value,
            }),
            Message::DecoderToggled(id) => {
                if !self.folded_decoders.remove(&id) {
                    self.folded_decoders.insert(id);
                }
                Task::none()
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
                self.open_submenu = None;
                self.open_menu = if self.open_menu == Some(index) {
                    None
                } else {
                    Some(index)
                };
                Task::none()
            }
            Message::MenuClosed => {
                self.open_menu = None;
                self.open_submenu = None;
                Task::none()
            }
            Message::DevicePicked(choice) => {
                let task = self.run(Command::SelectDevice(choice.id));
                self.sync_texts();
                task
            }
            Message::RatePicked(choice) => {
                if self.shell.state().activity != Activity::Idle {
                    return Task::none();
                }
                self.update(Message::RateText(choice.hz.to_string()))
            }
            Message::RateText(text) => {
                self.rate_text.clone_from(&text);
                match text.trim().parse::<u64>() {
                    Ok(hz) => {
                        let rate = self.run(Command::EditConfig(ConfigEdit::SampleRate(hz)));
                        // The length is kept in seconds: the samples follow the rate.
                        match la_host::config::samples_for_seconds(hz, &self.length_text) {
                            Ok(samples) => {
                                self.length_error = None;
                                let length =
                                    self.run(Command::EditConfig(ConfigEdit::Length(samples)));
                                Task::batch([rate, length])
                            }
                            Err(error) => {
                                self.length_error = Some(error);
                                rate
                            }
                        }
                    }
                    Err(_) => Task::none(),
                }
            }
            Message::LengthText(text) => {
                self.length_text.clone_from(&text);
                let rate = self.shell.state().config.sample_rate_hz;
                match la_host::config::samples_for_seconds(rate, &text) {
                    Ok(samples) => {
                        self.length_error = None;
                        self.run(Command::EditConfig(ConfigEdit::Length(samples)))
                    }
                    Err(error) => {
                        self.length_error = Some(error);
                        Task::none()
                    }
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
        let mut submenu: Option<(usize, Element<'_, Message>)> = None;
        for (position, (table_index, entry)) in self
            .table
            .iter()
            .enumerate()
            .filter(|(_, e)| e.menu[0] == *title)
            .enumerate()
        {
            let enabled = entry.enabled_when.is_enabled(self.shell.state());
            let mut label = match entry.accelerator {
                Some(a) => format!("{}    {}", self.t(entry.menu[1]), describe(a)),
                None => self.t(entry.menu[1]),
            };
            if matches!(entry.action, Action::AddDecoder) {
                label.push_str("  ▸");
            }
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
                    Action::AddDecoder => {
                        item = item.on_press(Message::SubmenuToggled(table_index));
                        if self.open_submenu == Some(table_index) {
                            submenu = Some((position, self.decoder_submenu()));
                        }
                    }
                }
            }
            items = items.push(item);
            if !enabled && matches!(entry.action, Action::AddDecoder) {
                items = items.push(text(self.t("decoders-need-capture")).size(11));
            }
        }
        if *title == "menu-file" {
            let recent = button(text(format!("{}  \u{25b8}", self.t("menu-file-recent"))))
                .style(button::text)
                .padding([4, 12])
                .width(Length::Shrink)
                .on_press(Message::SubmenuToggled(RECENT_SUBMENU));
            items = items.push(recent);
            if self.open_submenu == Some(RECENT_SUBMENU) {
                submenu = Some((table_count_in(&self.table, title), self.recent_submenu()));
            }
        }
        let offset = 6.0 + index as f32 * 90.0;
        let menu = container(items).style(container::bordered_box);
        let shown: Element<'_, Message> = match submenu {
            Some((position, submenu)) => row![
                menu,
                container(submenu).padding(iced::Padding {
                    top: position as f32 * 29.0,
                    ..iced::Padding::ZERO
                })
            ]
            .into(),
            None => menu.into(),
        };
        Some(
            container(shown)
                .padding(iced::Padding {
                    top: 30.0,
                    left: offset,
                    ..iced::Padding::ZERO
                })
                .into(),
        )
    }

    /// The submenu of File: the recent files with the folder as secondary text, then the entries
    /// that clean the list up.
    fn recent_submenu(&self) -> Element<'_, Message> {
        let recent = self.shell.recent_files();
        let idle = self.shell.state().operation.is_none();
        let mut items = column![].spacing(1).padding(4);
        if recent.is_empty() {
            items = items.push(
                button(text(self.t("menu-file-recent-empty")).size(13))
                    .style(button::text)
                    .padding([4, 12]),
            );
        }
        let mut any_missing = false;
        for file in &recent {
            let name = file.path.file_name().map_or_else(
                || file.path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            let folder = file
                .path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let label = if file.exists {
                name
            } else {
                any_missing = true;
                format!("{name}  {}", self.t("menu-file-recent-missing"))
            };
            let content = column![
                text(label).size(13),
                text(folder).size(10).style(text::secondary)
            ];
            let mut item = button(content)
                .style(button::text)
                .padding([3, 12])
                .width(Length::Shrink);
            if file.exists && idle {
                item = item.on_press(Message::Run(Command::OpenSession(file.path.clone())));
            }
            items = items.push(item);
        }
        if any_missing {
            items = items.push(
                button(text(self.t("menu-file-recent-remove-missing")).size(13))
                    .style(button::text)
                    .padding([4, 12])
                    .on_press(Message::RemoveMissingRecent),
            );
        }
        if !recent.is_empty() {
            items = items.push(
                button(text(self.t("menu-file-recent-clear")).size(13))
                    .style(button::text)
                    .padding([4, 12])
                    .on_press(Message::Run(Command::ClearRecentFiles)),
            );
        }
        container(items).style(container::bordered_box).into()
    }

    /// The decoders of the catalogue by name, in the order of the catalogue.
    fn decoder_submenu(&self) -> Element<'_, Message> {
        let mut items = column![].spacing(1).padding(4);
        for entry in &self.shell.state().decoders.catalogue {
            items = items.push(
                button(text(entry.name.clone()))
                    .style(button::text)
                    .padding([4, 12])
                    .width(Length::Shrink)
                    .on_press(Message::Run(Command::AddDecoder {
                        decoder: entry.id.clone(),
                    })),
            );
        }
        container(items).style(container::bordered_box).into()
    }

    /// The key D: opens the Decoders menu with the submenu of Add decoder expanded.
    fn open_add_decoder_submenu(&mut self) {
        let mut titles: Vec<&'static str> = Vec::new();
        for entry in &self.table {
            if !titles.contains(&entry.menu[0]) {
                titles.push(entry.menu[0]);
            }
        }
        let Some((table_index, entry)) = self
            .table
            .iter()
            .enumerate()
            .find(|(_, e)| matches!(e.action, Action::AddDecoder))
        else {
            return;
        };
        self.open_menu = titles.iter().position(|t| *t == entry.menu[0]);
        self.open_submenu = entry
            .enabled_when
            .is_enabled(self.shell.state())
            .then_some(table_index);
    }

    /// The width of the panel: the one being dragged, else the saved one, else the default.
    fn panel_width(&self) -> f32 {
        let (low, high) = la_host::settings::PANEL_WIDTH_RANGE;
        self.splitter.map_or_else(
            || {
                self.shell
                    .state()
                    .settings
                    .panel_width
                    .unwrap_or(la_host::settings::DEFAULT_PANEL_WIDTH)
                    .clamp(low, high) as f32
            },
            |drag| drag.width,
        )
    }

    /// Start (or Stop while capturing) and what keeps Start from working, at the top of the
    /// Capture view.
    fn start_block(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let idle = state.activity == Activity::Idle;
        let localizer = self.shell.localizer();
        // The button is as wide as the panel, like the fields below it; its text is centred.
        let label = |key: &'static str| {
            text(self.t(key))
                .width(Fill)
                .align_x(iced::alignment::Horizontal::Center)
        };
        let start = if idle {
            let mut b = button(label("toolbar-start")).width(Fill);
            if state.config_problems.is_empty() && state.selected_device.is_some() {
                b = b.on_press(Message::Run(Command::StartCapture));
            }
            b
        } else {
            button(label("toolbar-stop"))
                .width(Fill)
                .on_press(Message::Run(Command::StopCapture))
        };
        let mut toolbar = column![start].spacing(4);
        // "No channel is chosen" needs no message here: the channel buttons say it, and Start
        // stays unavailable.
        for problem in state
            .config_problems
            .iter()
            .filter(|p| p.setting != la_host::devices::Setting::Channels)
        {
            toolbar = toolbar.push(
                text(localizer.error_message_for_code(problem.code))
                    .size(12)
                    .style(text::danger),
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
        if idle {
            device_picker = device_picker.on_open(Message::Noop);
            rate = rate.on_input(Message::RateText);
            length = length.on_input(Message::LengthText);
            seed = seed.on_input(Message::SeedText);
        }
        let _ = &mut profile_picker;
        let localizer = self.shell.localizer();
        let samples_note = text(localizer.message(
            &Key::from_static("capture-length-samples"),
            &Args::new().uint("count", state.config.length),
        ))
        .size(12);
        let mut length_block = column![
            labelled(self.t("capture-length-seconds"), length.into()),
            samples_note
        ]
        .spacing(2);
        if let Some(error) = self.length_error {
            length_block = length_block.push(
                text(self.t(match error {
                    la_host::config::SecondsError::Empty
                    | la_host::config::SecondsError::NotANumber => "capture-length-invalid",
                    la_host::config::SecondsError::TooManyDecimals => "capture-length-decimals",
                    la_host::config::SecondsError::TooSmall => "capture-length-small",
                    la_host::config::SecondsError::TooLarge => "capture-length-large",
                    la_host::config::SecondsError::NoRate => "capture-length-no-rate",
                }))
                .size(12)
                .style(text::danger),
            );
        }
        // A device with a list of rates (a real board) gets a chooser of exactly those rates;
        // one that takes any rate in a range (the virtual device) gets a field to type it in.
        let rate_row = match state
            .devices
            .iter()
            .find(|d| Some(&d.id) == state.selected_device.as_ref())
            .map(|d| &d.capabilities.sample_rates)
        {
            Some(la_devices::SampleRates::List(rates)) => {
                let choices: Vec<RateChoice> =
                    rates.iter().map(|hz| RateChoice { hz: *hz }).collect();
                let current = choices
                    .iter()
                    .find(|c| c.hz == state.config.sample_rate_hz)
                    .cloned();
                labelled(
                    self.t("toolbar-rate-choice"),
                    pick_list(choices, current, Message::RatePicked)
                        .width(Fill)
                        .into(),
                )
            }
            _ => labelled(self.t("toolbar-rate"), rate.into()),
        };
        let mut view = column![
            text(self.t("panel-capture")).size(14),
            self.start_block(),
            labelled(self.t("toolbar-device"), device_picker.width(Fill).into()),
            rate_row,
            length_block,
            labelled(self.t("toolbar-seed"), seed.into()),
            labelled(self.t("toolbar-profile"), profile_picker.width(Fill).into()),
        ]
        .spacing(8)
        .padding(PANEL_PADDING);
        view = view.push(self.channel_buttons());
        scrollable(view).height(Fill).into()
    }

    /// The Markers view (see `marker_view`).
    fn markers_panel(&self) -> Element<'_, Message> {
        let texts = self.shell.marker_texts();
        let strings = self.marker_strings();
        crate::marker_view::Panel {
            state: self.shell.state(),
            texts: &texts,
            variant: self.variant,
            open: self.marker_menu,
            comments: &self.marker_comments,
            checked: &self.marker_checked,
            strings: &strings,
        }
        .view()
    }

    /// The panel of the decoders: the instances, their channels and options, the state of each
    /// pass and the selected annotation.
    fn decoders_panel(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let localizer = self.shell.localizer();
        let mut list = column![].spacing(6).padding(PANEL_PADDING);
        list = list.push(text(self.t("panel-decoders")).size(14));
        if state.capture.is_none() {
            list = list.push(text(self.t("decoders-need-capture")).size(12));
        } else {
            let catalogue = &state.decoders.catalogue;
            if let [only] = catalogue.as_slice() {
                list = list.push(
                    button(text(self.t("decoders-add")).size(13))
                        .padding([3, 8])
                        .on_press(Message::Run(Command::AddDecoder {
                            decoder: only.id.clone(),
                        })),
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
        // The decoder is shown by its name; a name given through the host API is kept.
        let name = if instance.setup.name.is_empty() {
            entry.map_or_else(String::new, |e| e.name.clone())
        } else {
            instance.setup.name.clone()
        };
        let folded = self.folded_decoders.contains(&id);
        let state_key = match instance.state {
            InstanceState::Waiting => "decoder-state-waiting",
            InstanceState::Running => "decoder-state-running",
            InstanceState::Finished => "decoder-state-finished",
            InstanceState::Cancelled => "decoder-state-cancelled",
            InstanceState::Failed => "decoder-state-failed",
        };
        let mut block = column![
            row![
                button(icons::icon(
                    if folded {
                        Icon::ChevronRight
                    } else {
                        Icon::ChevronDown
                    },
                    false,
                ))
                .style(button::text)
                .padding(2)
                .on_press(Message::DecoderToggled(id)),
                text(name).size(14).width(Fill),
                button(icons::icon(Icon::Trash, false))
                    .style(button::text)
                    .padding(2)
                    .on_press(Message::Run(Command::RemoveDecoder(id))),
            ]
            .spacing(4)
            .align_y(iced::Alignment::Center)
        ]
        .spacing(3);
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
        if let Some(code) = instance.fault {
            block = block.push(
                text(localizer.error_message_for_code(code))
                    .size(12)
                    .style(text::danger),
            );
        } else if instance.incomplete {
            block = block.push(text(self.t("decoder-incomplete")).size(12));
        }
        if folded {
            return block.into();
        }
        block = block.push(
            checkbox(instance.setup.lane_visible)
                .label(self.t("decoder-lane-show"))
                .size(14)
                .on_toggle(move |visible| Message::Run(Command::ShowDecoderLane { id, visible })),
        );
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

    /// How many rows of eight buttons `channels` channels need.
    fn button_rows(channels: usize) -> usize {
        channels.div_ceil(8)
    }

    /// The channels to record, as buttons eight in a row: a channel that is off shows its number and
    /// its border in its colour, one that is on is filled with its colour.
    fn channel_buttons(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let variant = self.variant;
        // The buttons are in the order of the channel numbers, not of the display order.
        let mut styles: Vec<&la_host::presentation::ChannelStyle> =
            state.presentation.channels.iter().collect();
        styles.sort_by_key(|s| s.index);
        let can_flag = state.activity == Activity::Idle;
        let mut rows = column![].spacing(CHANNEL_GAP);
        debug_assert_eq!(styles.chunks(8).count(), Self::button_rows(styles.len()));
        for chunk in styles.chunks(8) {
            let mut line = row![].spacing(CHANNEL_GAP);
            for style in chunk {
                let (index, on, colour) = (style.index, style.visible, style.colour);
                let label = self.shell.localizer().message(
                    &Key::from_static("channel-default"),
                    &Args::new().uint("number", u64::from(index)),
                );
                let name = if style.name.is_empty() {
                    label
                } else {
                    style.name.clone()
                };
                let press = button(
                    container(text(index.to_string()).size(12))
                        .center_x(Fill)
                        .center_y(Fill),
                )
                .width(Fill)
                .height(CHANNEL_BUTTON)
                .padding(0)
                .style(move |theme: &Theme, status| {
                    crate::theme::channel_button_style(variant, colour, on, theme, status)
                })
                .on_press_maybe(can_flag.then_some(Message::ChannelVisible(index, !on)));
                line = line.push(
                    tooltip(
                        press,
                        container(text(name).size(12))
                            .padding(4)
                            .style(container::bordered_box),
                        tooltip::Position::Bottom,
                    )
                    .style(|_: &Theme| container::Style::default()),
                );
            }
            // A short last row keeps the widths of the full ones.
            for _ in chunk.len()..8 {
                line = line.push(Space::new().width(Fill));
            }
            rows = rows.push(line.width(Fill));
        }
        let mut block = column![text(self.t("capture-channels")).size(13), rows].spacing(4);
        if state.config.enabled_channels == 0 {
            block = block.push(
                text(self.t("capture-no-channel"))
                    .size(12)
                    .style(text::danger),
            );
        }
        let small = |label: String, message: Message| {
            button(
                text(label)
                    .size(13)
                    .width(Fill)
                    .align_x(iced::alignment::Horizontal::Center),
            )
            .width(Fill)
            .style(button::secondary)
            .on_press_maybe(can_flag.then_some(message))
        };
        block = block.push(
            row![
                small(
                    self.t("channel-reset"),
                    Message::Run(Command::ResetPresentation)
                ),
                small(self.t("capture-channels-all"), Message::ChannelsAll(true)),
                small(self.t("capture-channels-none"), Message::ChannelsAll(false)),
            ]
            .spacing(4)
            .width(Fill),
        );
        block.into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let line = self.shell.status_line();
        // The numbers that change while a capture runs are shown in a monospaced font and padded to
        // the widest they have been, so the text does not shift as the digits change.
        let capturing = self.shell.state().activity == Activity::Capturing;
        let mut widths = if capturing {
            self.status_widths.get()
        } else {
            [0; 3]
        };
        let steady = steady_parts(&mut widths, [line.elapsed, line.samples, line.memory]);
        self.status_widths.set(widths);
        let [elapsed, samples, memory] = steady;
        let separator = || text("  \u{b7}  ").size(13);
        let mut cells = row![text(line.activity).size(13)];
        for part in [elapsed, samples, memory].into_iter().flatten() {
            cells = cells
                .push(separator())
                .push(text(part).size(13).font(iced::Font::MONOSPACE));
        }
        let mut rest: Vec<String> = Vec::new();
        rest.extend(line.hint);
        rest.extend(line.end_reason);
        rest.extend(line.rate);
        if line.simulated {
            rest.push(self.t("status-simulated"));
        }
        if line.incomplete {
            rest.push(self.t("status-incomplete"));
        }
        if line.unsaved {
            rest.push(self.t("status-unsaved"));
        }
        for part in rest {
            cells = cells.push(separator()).push(text(part).size(13));
        }
        container(cells)
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
            la_host::OperationKind::Crop => self.t("dialog-cropping"),
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

    /// The question before data of the capture is deleted: what goes, what stays, and that it
    /// cannot be undone.
    fn crop_dialog(&self, plan: la_host::CropPlan) -> Element<'_, Message> {
        let state = self.shell.state();
        let localizer = self.shell.localizer();
        let rate = state
            .capture
            .as_ref()
            .map_or(1, |c| c.snapshot.sample_rate_hz())
            .max(1) as f64;
        let time = |samples: u64| localizer.format_duration(samples as f64 / rate);
        let percent = plan
            .lost()
            .saturating_mul(100)
            .checked_div(plan.total)
            .unwrap_or(0);
        let key = match plan.side {
            la_host::CropSide::Left => "dialog-crop-left-body",
            la_host::CropSide::Right => "dialog-crop-right-body",
            la_host::CropSide::Pair => "dialog-crop-pair-body",
        };
        let message = localizer.message(
            &Key::from_static(key),
            &Args::new()
                .trusted("lost", &time(plan.lost()))
                .trusted("kept", &time(plan.kept()))
                .uint("percent", percent),
        );
        let body = column![
            text(self.t("dialog-crop-title")).size(18),
            text(message),
            row![
                button(text(self.t("dialog-crop-confirm")))
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

    fn unsaved_dialog(&self) -> Element<'_, Message> {
        if let Some(la_host::Guarded::Crop(plan)) = self.shell.state().pending_decision {
            return self.crop_dialog(plan);
        }
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
            checkbox(self.remember_choice)
                .label(self.t("settings-remember-choice"))
                .size(14)
                .on_toggle(Message::RememberToggled),
            row![
                button(text(self.t("dialog-save"))).on_press_maybe(
                    (self.shell.state().activity == Activity::Idle
                        && self.shell.state().operation.is_none())
                    .then_some(Message::UnsavedAnswer(Decision::Save))
                ),
                button(text(self.t("dialog-discard")))
                    .style(button::danger)
                    .on_press(Message::UnsavedAnswer(Decision::Discard)),
                button(text(self.t("dialog-cancel")))
                    .style(button::secondary)
                    .on_press(Message::UnsavedAnswer(Decision::Cancel)),
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
        let started = Instant::now();
        let element = self.view_tree();
        crate::frame_log::record(crate::frame_log::Stage::View, started.elapsed());
        element
    }

    fn view_tree(&self) -> Element<'_, Message> {
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
            placement: self.shell.state().placement,
            preview: self.placement_preview,
            drag_end: self.shell.state().drag,
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
                        top: edit.top,
                        left: 8.0,
                        ..iced::Padding::ZERO
                    })
                ]
                .into()
            }
            None => signal.into(),
        };

        let mut content = column![self.menu_bar()];
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
            body = body.push(crate::side_panel::frame(panel, self.panel_width()));
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
    /// The pointer that means `sample` in the window now shown, in the middle of its column.
    fn pointer_for(app: &App, sample: u64) -> la_host::markers::Pointer {
        let state = app.shell.state();
        let length = state.capture.as_ref().map_or(1, |c| c.snapshot.length());
        let (start, window_length) = if state.auto_fit {
            (0, length.max(1))
        } else {
            (state.window.start, state.window.length.max(1))
        };
        let pixels = f64::from(state.view_size.pixels);
        la_host::markers::Pointer {
            x: ((sample.saturating_sub(start) as f64 + 0.5) * pixels / window_length as f64) as f32,
            lane: None,
        }
    }

    /// Places a single marker the way the user does: start, then a click.
    fn place_single(app: &mut App, sample: u64) {
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Single,
        )));
        let pointer = pointer_for(app, sample);
        let _ = app.update(Message::Run(Command::FixPlacement { pointer }));
    }

    /// Places a pair marker: start, then two clicks.
    fn place_pair(app: &mut App, a: u64, b: u64) {
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Pair,
        )));
        for sample in [a, b] {
            let pointer = pointer_for(app, sample);
            let _ = app.update(Message::Run(Command::FixPlacement { pointer }));
        }
    }

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
    fn eight_channel_buttons_fit_a_row_of_the_narrowest_panel() {
        // The content has the same room on both sides; the buttons share what is left.
        let (narrowest, _) = la_host::settings::PANEL_WIDTH_RANGE;
        let room = narrowest as f32 - PANEL_PADDING.left - PANEL_PADDING.right - 7.0 * CHANNEL_GAP;
        assert!(
            room / 8.0 >= 20.0,
            "a button would be {} px wide",
            room / 8.0
        );
    }

    #[test]
    fn start_is_in_the_capture_view_and_not_in_a_toolbar() {
        let _turn = render_turn();
        let mut app = app_with_capture("start-in-panel");
        let start = app.t("toolbar-start");
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Capture));
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(900.0, 600.0),
            app.view(),
        );
        assert!(
            ui.find(start.as_str()).is_ok(),
            "Start is in the Capture view"
        );
        drop(ui);
        // A slow capture is running: Stop replaces Start.
        for edit in [
            ConfigEdit::Realtime(true),
            ConfigEdit::SampleRate(1_000),
            ConfigEdit::Length(100_000),
        ] {
            let _ = app.update(Message::Run(Command::EditConfig(edit)));
        }
        let outcome = app.shell.apply(Command::StartCapture);
        if matches!(outcome, Outcome::NeedsDecision(_)) {
            let _ = app.shell.apply(Command::Decide(Decision::Discard));
        }
        assert_eq!(app.shell.state().activity, Activity::Capturing);
        let stop = app.t("toolbar-stop");
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(900.0, 600.0),
            app.view(),
        );
        assert!(
            ui.find(stop.as_str()).is_ok(),
            "Stop replaces it while capturing"
        );
        drop(ui);
        // With another view shown the panel has no Start.
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Markers));
        let ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(900.0, 600.0),
            app.view(),
        );
        let mut ui = ui;
        assert!(ui.find(stop.as_str()).is_err());
    }

    #[test]
    fn all_and_none_switch_every_channel_at_once() {
        let _turn = render_turn();
        let mut app = app_with_capture("all-none");
        let flags = |app: &App| -> Vec<bool> {
            let mut list: Vec<_> = app
                .shell
                .state()
                .presentation
                .channels
                .iter()
                .map(|c| (c.index, c.visible))
                .collect();
            list.sort_unstable();
            list.into_iter().map(|(_, v)| v).collect()
        };
        let _ = app.update(Message::ChannelsAll(false));
        assert!(flags(&app).iter().all(|v| !v));
        assert_eq!(app.shell.state().config.enabled_channels, 0);
        let _ = app.update(Message::ChannelsAll(true));
        assert!(flags(&app).iter().all(|v| *v));
        assert_eq!(
            app.shell.state().config.enabled_channels.count_ones() as usize,
            app.shell.state().presentation.channels.len().min(32)
        );
        // The buttons and the two new ones render in three languages.
        for language in [
            la_i18n::Language::En,
            la_i18n::Language::De,
            la_i18n::Language::Ja,
        ] {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(800.0, 560.0),
                app.view(),
            );
            for key in [
                "capture-channels-all",
                "capture-channels-none",
                "channel-reset",
            ] {
                let label = app.t(key);
                assert!(ui.find(label.as_str()).is_ok(), "{language:?} {key}");
            }
        }
    }

    #[test]
    fn a_comma_and_a_point_both_separate_the_fraction_of_the_length_in_every_language() {
        let mut app = app_with_capture("length-comma");
        for language in la_i18n::Language::ALL {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            for text in ["1.5", "1,5"] {
                let _ = app.update(Message::RateText("1000".to_owned()));
                let _ = app.update(Message::LengthText(text.to_owned()));
                assert_eq!(
                    app.shell.state().config.length,
                    1_500,
                    "{language:?} {text}"
                );
                assert!(app.length_error.is_none(), "{language:?} {text}");
            }
        }
    }

    #[test]
    fn a_decoder_is_shown_by_its_name_and_its_properties_fold_away() {
        let _turn = render_turn();
        let mut app = app_with_capture("decoder-fold");
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Decoders));
        let id = app.shell.state().decoders.catalogue[0].id.clone();
        let name = app.shell.state().decoders.catalogue[0].name.clone();
        let _ = app.update(Message::Run(Command::AddDecoder { decoder: id }));
        let instance = app.shell.state().decoders.instances[0].id;
        let properties = app.t("decoder-lane-show");
        let find = |app: &App, label: &str| {
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(1180.0, 720.0),
                app.view(),
            );
            ui.find(label).is_ok()
        };
        // The name is plain text (the decoder is not renamed in the panel) and the properties show.
        assert!(find(&app, &name), "{name}");
        assert!(find(&app, &properties));
        // Folded: the header and the state stay, the properties go.
        let _ = app.update(Message::DecoderToggled(instance));
        assert!(app.folded_decoders.contains(&instance));
        assert!(find(&app, &name));
        assert!(!find(&app, &properties));
        // Unfolded again.
        let _ = app.update(Message::DecoderToggled(instance));
        assert!(find(&app, &properties));
        // The bin removes it (a decoder without results is removed at once).
        let _ = app.update(Message::Run(Command::RemoveDecoder(instance)));
        assert!(app.shell.state().decoders.instances.is_empty());
    }

    #[test]
    fn the_decoder_header_renders_in_every_language_at_the_minimum_size() {
        let _turn = render_turn();
        let mut app = app_with_capture("decoder-header");
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Decoders));
        let id = app.shell.state().decoders.catalogue[0].id.clone();
        let _ = app.update(Message::Run(Command::AddDecoder { decoder: id }));
        for language in la_i18n::Language::ALL {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            for _ in 0..2 {
                let ui = iced_test::Simulator::with_size(
                    crate::fonts::settings(),
                    iced::Size::new(800.0, 560.0),
                    app.view(),
                );
                drop(ui);
                let instance = app.shell.state().decoders.instances[0].id;
                let _ = app.update(Message::DecoderToggled(instance));
            }
        }
    }

    #[test]
    fn the_question_about_an_unsaved_capture_can_remember_the_answer() {
        let _turn = render_turn();
        let mut app = app_with_capture("remember-dialog");
        let remember = app.t("settings-remember-choice");
        // The dialog has the box, unticked.
        let _ = app.update(Message::Run(Command::NewCapture));
        assert!(app.shell.state().pending_decision.is_some());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(800.0, 560.0),
            app.view(),
        );
        assert!(ui.find(remember.as_str()).is_ok());
        drop(ui);
        assert!(!app.remember_choice);
        // An answer with the box unticked is not remembered, and Cancel never is.
        let _ = app.update(Message::UnsavedAnswer(Decision::Cancel));
        assert!(app.shell.state().settings.unsaved_choice.is_none());
        assert!(app.shell.state().capture.is_some());
        let _ = app.update(Message::Run(Command::NewCapture));
        let _ = app.update(Message::RememberToggled(true));
        let _ = app.update(Message::UnsavedAnswer(Decision::Cancel));
        assert!(app.shell.state().settings.unsaved_choice.is_none());
        assert!(!app.remember_choice, "the box is clear again");
        // With the box ticked it is, and the question is not asked again.
        let _ = app.update(Message::Run(Command::NewCapture));
        let _ = app.update(Message::RememberToggled(true));
        let _ = app.update(Message::UnsavedAnswer(Decision::Discard));
        assert_eq!(
            app.shell.state().settings.unsaved_choice.as_deref(),
            Some("discard")
        );
        assert!(!app.remember_choice, "the box is clear again");
        assert!(app.shell.state().capture.is_none());
        let mut again = app_with_capture("remember-dialog-again");
        let _ = again.update(Message::Run(Command::SetUnsavedChoice(Some(
            Decision::Discard,
        ))));
        let _ = again.update(Message::Run(Command::NewCapture));
        assert!(again.shell.state().pending_decision.is_none());
        assert!(again.shell.state().capture.is_none());
    }

    #[test]
    fn the_settings_have_the_box_and_the_two_answers_that_can_be_remembered() {
        let _turn = render_turn();
        let mut app = app_with_capture("remember-settings");
        let _ = app.update(Message::OpenSettings);
        let answers = [app.t("dialog-save"), app.t("dialog-discard")];
        let has = |app: &App, label: &str| {
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(800.0, 700.0),
                app.view(),
            );
            ui.find(label).is_ok()
        };
        let remember = app.t("settings-remember-choice");
        assert!(has(&app, &remember));
        // Nothing is remembered: the three answers are not offered.
        assert!(app.shell.state().settings.unsaved_choice.is_none());
        // Ticking the box remembers Save, and the choice can be changed.
        let _ = app.update(Message::RememberInSettings(true));
        assert_eq!(
            app.shell.state().settings.unsaved_choice.as_deref(),
            Some("save")
        );
        for answer in &answers {
            assert!(has(&app, answer), "{answer}");
        }
        let _ = app.update(Message::RememberedAnswer(Decision::Discard));
        assert_eq!(
            app.shell.state().settings.unsaved_choice.as_deref(),
            Some("discard")
        );
        // Cancel is not one of the answers: it is not offered, and the host would refuse it.
        let _ = app.update(Message::RememberedAnswer(Decision::Cancel));
        assert_eq!(
            app.shell.state().settings.unsaved_choice.as_deref(),
            Some("discard")
        );
        // Unticking goes back to asking.
        let _ = app.update(Message::RememberInSettings(false));
        assert!(app.shell.state().settings.unsaved_choice.is_none());
        // Every language shows it at the minimum size.
        let _ = app.update(Message::RememberInSettings(true));
        for language in la_i18n::Language::ALL {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            let label = app.t("settings-remember-choice");
            assert!(has(&app, &label), "{language:?}");
        }
    }

    #[test]
    fn the_splitter_changes_the_width_of_the_panel_within_its_range_and_keeps_it() {
        use la_host::settings::{DEFAULT_PANEL_WIDTH, PANEL_WIDTH_RANGE};
        let _turn = render_turn();
        let mut app = app_with_capture("splitter");
        assert!((app.panel_width() - DEFAULT_PANEL_WIDTH as f32).abs() < f32::EPSILON);
        // Dragging to the left widens the panel; the first move only sets where the drag began.
        let _ = app.update(Message::SplitterPressed);
        let _ = app.update(Message::SplitterMoved(900.0));
        assert!((app.panel_width() - DEFAULT_PANEL_WIDTH as f32).abs() < f32::EPSILON);
        let _ = app.update(Message::SplitterMoved(850.0));
        assert!((app.panel_width() - (DEFAULT_PANEL_WIDTH as f32 + 50.0)).abs() < f32::EPSILON);
        // Never beyond the range, either way.
        let _ = app.update(Message::SplitterMoved(-5000.0));
        assert!((app.panel_width() - PANEL_WIDTH_RANGE.1 as f32).abs() < f32::EPSILON);
        let _ = app.update(Message::SplitterMoved(5000.0));
        assert!((app.panel_width() - PANEL_WIDTH_RANGE.0 as f32).abs() < f32::EPSILON);
        // The width is kept only when the button is released.
        assert_eq!(app.shell.state().settings.panel_width, None);
        let _ = app.update(Message::SplitterMoved(860.0));
        let _ = app.update(Message::SplitterReleased);
        assert!(app.splitter.is_none());
        assert_eq!(
            app.shell.state().settings.panel_width,
            Some(DEFAULT_PANEL_WIDTH + 40)
        );
        assert!((app.panel_width() - (DEFAULT_PANEL_WIDTH + 40) as f32).abs() < f32::EPSILON);
        // Moves and a release without a press change nothing.
        let _ = app.update(Message::SplitterMoved(10.0));
        let _ = app.update(Message::SplitterReleased);
        assert_eq!(
            app.shell.state().settings.panel_width,
            Some(DEFAULT_PANEL_WIDTH + 40)
        );
    }

    #[test]
    fn the_panel_and_its_buttons_render_at_the_narrowest_and_the_widest_width() {
        use la_host::settings::PANEL_WIDTH_RANGE;
        let _turn = render_turn();
        let mut app = app_with_capture("splitter-render");
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Capture));
        for width in [PANEL_WIDTH_RANGE.0, PANEL_WIDTH_RANGE.1] {
            let _ = app.update(Message::Run(Command::SetPanelWidth(width)));
            for language in [la_i18n::Language::En, la_i18n::Language::De] {
                let _ = app.update(Message::Run(Command::SetLanguage(language)));
                let mut ui = iced_test::Simulator::with_size(
                    crate::fonts::settings(),
                    iced::Size::new(800.0, 600.0),
                    app.view(),
                );
                for key in [
                    "capture-channels-all",
                    "capture-channels-none",
                    "channel-reset",
                ] {
                    let label = app.t(key);
                    let found = ui.find(label.as_str()).unwrap();
                    let bounds = found.bounds();
                    assert!(
                        bounds.x >= 0.0 && bounds.x + bounds.width <= 800.0,
                        "{width} {language:?} {key}: {bounds:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_application_registers_the_driver_of_the_weact_analyzer() {
        let providers = device_providers();
        let names: Vec<&str> = providers.iter().map(|p| p.name()).collect();
        assert_eq!(names, vec!["weact"]);
    }

    #[test]
    fn the_channel_buttons_are_eight_to_a_row() {
        assert_eq!(App::button_rows(0), 0);
        assert_eq!(App::button_rows(1), 1);
        assert_eq!(App::button_rows(8), 1);
        assert_eq!(App::button_rows(9), 2);
        assert_eq!(App::button_rows(16), 2);
        assert_eq!(App::button_rows(32), 4);
    }

    #[test]
    fn the_capture_view_renders_in_three_languages_at_the_minimum_size_and_while_recording() {
        let _turn = render_turn();
        let mut app = app_with_capture("capture-view");
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Capture));
        for language in [
            la_i18n::Language::En,
            la_i18n::Language::De,
            la_i18n::Language::Ja,
        ] {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(800.0, 560.0),
                app.view(),
            );
            for key in ["capture-length-seconds", "capture-channels", "toolbar-rate"] {
                let label = app.t(key);
                assert!(
                    ui.find(label.as_str()).is_ok(),
                    "{language:?} {key}: {label}"
                );
            }
            let shot = ui.snapshot(&app.theme()).unwrap();
            if let Ok(directory) = std::env::var("LA_SNAPSHOT_DIR") {
                shot.matches_image(
                    std::path::Path::new(&directory).join(format!("capture-{}", language.tag())),
                )
                .unwrap();
            }
        }
        // With a capture running the buttons are inert and the view still renders.
        for edit in [
            ConfigEdit::Realtime(true),
            ConfigEdit::SampleRate(1_000),
            ConfigEdit::Length(100_000),
        ] {
            let _ = app.update(Message::Run(Command::EditConfig(edit)));
        }
        let outcome = app.shell.apply(Command::StartCapture);
        if matches!(outcome, Outcome::NeedsDecision(_)) {
            let _ = app.shell.apply(Command::Decide(Decision::Discard));
        }
        assert_eq!(app.shell.state().activity, Activity::Capturing);
        let before = app.shell.state().config.enabled_channels;
        let _ = app.update(Message::ChannelVisible(7, true));
        assert_eq!(app.shell.state().config.enabled_channels, before);
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(800.0, 560.0),
            app.view(),
        );
        let _ = ui.snapshot(&app.theme()).unwrap();
        drop(ui);
        let _ = app.shell.apply(Command::StopCapture);
    }

    #[test]
    fn the_numbers_of_the_status_line_keep_their_width_while_the_digits_change() {
        let mut widths = [0; 3];
        let mut seen = Vec::new();
        for (elapsed, samples) in [
            ("0.9 s", "9 samples"),
            ("1.0 s", "10 samples"),
            ("1.1 s", "1,000 samples"),
            ("2.0 s", "999 samples"),
            ("10.0 s", "1,000,000 samples"),
            ("10.1 s", "9 samples"),
        ] {
            let [e, s, m] = steady_parts(
                &mut widths,
                [Some(elapsed.to_owned()), Some(samples.to_owned()), None],
            );
            assert!(m.is_none(), "a part that is missing stays missing");
            seen.push((columns(&e.unwrap()), columns(&s.unwrap())));
        }
        // The widths never shrink, and a value shorter than the widest is padded to it.
        for pair in seen.windows(2) {
            assert!(pair[1].0 >= pair[0].0 && pair[1].1 >= pair[0].1, "{seen:?}");
        }
        assert_eq!(
            seen[5], seen[4],
            "a shorter value after the longest changes nothing"
        );
        assert_eq!(widths, [columns("10.0 s"), columns("1,000,000 samples"), 0]);
    }

    #[test]
    fn a_text_of_wide_characters_counts_two_columns_each() {
        assert_eq!(columns("12 ms"), 5);
        assert_eq!(columns("\u{30b5}\u{30f3}"), 4);
    }

    #[test]
    fn the_start_button_is_as_wide_as_the_panel() {
        let _turn = render_turn();
        let mut app = app_with_capture("start-width");
        let start = app.t("toolbar-start");
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Capture));
        for width in [220_u32, 280, 480] {
            let _ = app.update(Message::Run(Command::SetPanelWidth(width)));
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(900.0, 600.0),
                app.view(),
            );
            let bounds = ui.find(start.as_str()).unwrap().bounds();
            // The text fills the button and the button fills the panel: far wider than the word.
            assert!(
                bounds.width > f32::from(u16::try_from(width).unwrap()) - 80.0,
                "{width}: {bounds:?}"
            );
        }
    }

    #[test]
    fn rates_are_shown_in_the_unit_a_person_reads() {
        for (hz, text) in [
            (20_000, "20 kHz"),
            (250_000, "250 kHz"),
            (1_000_000, "1 MHz"),
            (24_000_000, "24 MHz"),
            (1_500_000, "1500 kHz"),
            (999, "999 Hz"),
            (0, "0 Hz"),
        ] {
            assert_eq!(format_rate(hz), text);
            assert_eq!(RateChoice { hz }.to_string(), text);
        }
    }

    #[test]
    fn a_rate_chosen_from_the_list_sets_the_rate_and_keeps_the_length_in_seconds() {
        let _turn = render_turn();
        let dir = std::env::temp_dir().join(format!("la-app-rate-pick-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        let _ = app.update(Message::RateText("1000000".into()));
        let _ = app.update(Message::LengthText("2.5".into()));
        let _ = app.update(Message::RatePicked(RateChoice { hz: 2_000_000 }));
        assert_eq!(app.shell.state().config.sample_rate_hz, 2_000_000);
        assert_eq!(app.rate_text, "2000000");
        assert_eq!(app.shell.state().config.length, 5_000_000);
    }

    #[test]
    fn the_length_is_typed_in_seconds_and_follows_the_sample_rate() {
        let _turn = render_turn();
        let dir = std::env::temp_dir().join(format!("la-app-seconds-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        let _ = app.update(Message::RateText("1000000".into()));
        let _ = app.update(Message::LengthText("2.5".into()));
        assert_eq!(app.shell.state().config.length, 2_500_000);
        assert!(app.length_error.is_none());
        // A new rate keeps the seconds and recomputes the samples.
        let _ = app.update(Message::RateText("2000000".into()));
        assert_eq!(app.length_text, "2.5");
        assert_eq!(app.shell.state().config.length, 5_000_000);
        // Text that is not a length sends nothing and says why.
        for (text, error) in [
            ("abc", la_host::config::SecondsError::NotANumber),
            ("", la_host::config::SecondsError::Empty),
            ("0.0000001", la_host::config::SecondsError::TooSmall),
            (
                "0.1234567891",
                la_host::config::SecondsError::TooManyDecimals,
            ),
        ] {
            let _ = app.update(Message::LengthText(text.into()));
            assert_eq!(app.length_error, Some(error), "{text:?}");
            assert_eq!(app.shell.state().config.length, 5_000_000, "{text:?}");
        }
        // A good text clears the message again.
        let _ = app.update(Message::LengthText("1".into()));
        assert!(app.length_error.is_none());
        assert_eq!(app.shell.state().config.length, 2_000_000);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_side_panel_shows_one_view_at_a_time_or_none_in_every_state() {
        let _turn = render_turn();
        let mut app = app_with_capture("side-panel");
        // A text that only that view has: not a menu title and not in the other views.
        let marks = [
            (la_host::SidePanel::Capture, app.t("toolbar-rate")),
            (la_host::SidePanel::Decoders, app.t("decoders-empty")),
            (la_host::SidePanel::Markers, app.t("marker-empty")),
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
        // The menu lists every decoder by name and adds the one that is chosen.
        press_d(&mut app);
        assert!(app.open_menu.is_some() && app.open_submenu.is_some());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        for entry in &app.shell.state().decoders.catalogue {
            assert!(ui.find(entry.name.as_str()).is_ok(), "{}", entry.name);
        }
        drop(ui);
        let _ = app.update(Message::Run(Command::AddDecoder {
            decoder: "test.echo".to_owned(),
        }));
        assert_eq!(app.shell.state().decoders.instances.len(), 2);
        assert!(app.open_menu.is_none() && app.open_submenu.is_none());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        ui.snapshot(&app.theme()).unwrap();
    }

    /// The key D, as the keyboard sends it.
    fn press_d(app: &mut App) {
        let key = keyboard::Key::Character("d".into());
        let _ = app.update(Message::Key(keyboard::Event::KeyPressed {
            key: key.clone(),
            modified_key: key,
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::KeyD),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: Some("d".into()),
            repeat: false,
        }));
    }

    /// The index of the Decoders menu in the menu bar and of the entry Add decoder in the table.
    fn decoders_menu(app: &App) -> (usize, usize) {
        let entry = app
            .table
            .iter()
            .position(|e| matches!(e.action, Action::AddDecoder))
            .unwrap();
        let mut titles: Vec<&'static str> = Vec::new();
        for e in &app.table {
            if !titles.contains(&e.menu[0]) {
                titles.push(e.menu[0]);
            }
        }
        let menu = titles
            .iter()
            .position(|t| *t == app.table[entry].menu[0])
            .unwrap();
        (menu, entry)
    }

    #[test]
    fn add_decoder_in_the_menu_lists_the_decoders_by_name_and_adds_the_chosen_one() {
        let _turn = render_turn();
        let mut app = app_with_capture("add-decoder-menu");
        let (menu, entry) = decoders_menu(&app);
        let names: Vec<(String, String)> = app
            .shell
            .state()
            .decoders
            .catalogue
            .iter()
            .map(|c| (c.id.clone(), c.name.clone()))
            .collect();
        assert_eq!(names.len(), 2);
        for (id, _) in &names {
            let _ = app.update(Message::MenuToggled(menu));
            assert!(app.open_submenu.is_none(), "opening a menu starts closed");
            let _ = app.update(Message::SubmenuToggled(entry));
            assert_eq!(app.open_submenu, Some(entry));
            let _ = app.update(Message::Run(Command::AddDecoder {
                decoder: id.clone(),
            }));
            assert!(app.open_menu.is_none() && app.open_submenu.is_none());
        }
        let added: Vec<String> = app
            .shell
            .state()
            .decoders
            .instances
            .iter()
            .map(|i| i.setup.decoder.clone())
            .collect();
        assert_eq!(added, names.iter().map(|n| n.0.clone()).collect::<Vec<_>>());
        // A second press on the entry closes the submenu again.
        let _ = app.update(Message::MenuToggled(menu));
        let _ = app.update(Message::SubmenuToggled(entry));
        let _ = app.update(Message::SubmenuToggled(entry));
        assert!(app.open_submenu.is_none());
    }

    #[test]
    fn the_key_d_opens_the_decoders_menu_with_the_submenu_expanded() {
        let _turn = render_turn();
        let mut app = app_with_capture("key-d");
        let (menu, entry) = decoders_menu(&app);
        press_d(&mut app);
        assert_eq!(app.open_menu, Some(menu));
        assert_eq!(app.open_submenu, Some(entry));
        // Three and more registered decoders are all listed.
        app.shell
            .register_decoder(&ECHO, la_decoders::decoders::clocked_bits::create)
            .unwrap();
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(800.0, 560.0),
            app.view(),
        );
        for entry in &app.shell.state().decoders.catalogue {
            assert!(ui.find(entry.name.as_str()).is_ok(), "{}", entry.name);
        }
    }

    #[test]
    fn without_a_capture_add_decoder_is_disabled_and_says_why() {
        let _turn = render_turn();
        let dir = std::env::temp_dir().join(format!("la-app-nocap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::with_shell(Shell::new(ShellConfig::for_tests(&dir)));
        let (menu, _) = decoders_menu(&app);
        // The key of a disabled entry does nothing; the menu shows the entry and the reason.
        press_d(&mut app);
        assert!(app.open_menu.is_none());
        let _ = app.update(Message::MenuToggled(menu));
        assert!(app.open_submenu.is_none());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(800.0, 560.0),
            app.view(),
        );
        let reason = app.t("decoders-need-capture");
        assert!(ui.find(reason.as_str()).is_ok(), "{reason}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_submenu_is_inside_the_window_at_the_minimum_size_in_english_and_german() {
        let _turn = render_turn();
        let mut app = app_with_capture("submenu-min");
        for language in [la_i18n::Language::En, la_i18n::Language::De] {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            press_d(&mut app);
            assert!(app.open_submenu.is_some());
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(800.0, 560.0),
                app.view(),
            );
            for entry in &app.shell.state().decoders.catalogue {
                let found = ui.find(entry.name.as_str()).unwrap();
                let bounds = found.bounds();
                assert!(
                    bounds.x >= 0.0
                        && bounds.y >= 0.0
                        && bounds.x + bounds.width <= 800.0
                        && bounds.y + bounds.height <= 560.0,
                    "{language:?} {}: {bounds:?}",
                    entry.name
                );
            }
            drop(ui);
            let _ = app.update(Message::MenuClosed);
        }
    }

    /// An app that has saved `count` sessions `r0` .. in a scratch directory.
    fn app_with_recent(
        name: &str,
        count: usize,
    ) -> (App, std::path::PathBuf, Vec<std::path::PathBuf>) {
        let mut app = app_with_capture(name);
        let dir = std::env::temp_dir().join(format!("la-app-{name}-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut paths = Vec::new();
        for i in 0..count {
            let path = dir.join(format!("r{i}.lasession"));
            let _ = app.update(Message::Run(Command::SaveSessionAs(path.clone())));
            let deadline = Instant::now().checked_add(Duration::from_secs(20)).unwrap();
            while app.shell.operation_running() {
                let _ = app.update(Message::Tick(Instant::now()));
                assert!(Instant::now() < deadline, "the save did not end");
                std::thread::sleep(Duration::from_millis(5));
            }
            paths.push(path);
        }
        (app, dir, paths)
    }

    fn open_recent(app: &mut App) {
        let file = app
            .table
            .iter()
            .position(|e| e.menu[0] == "menu-file")
            .unwrap();
        let mut titles: Vec<&'static str> = Vec::new();
        for e in &app.table {
            if !titles.contains(&e.menu[0]) {
                titles.push(e.menu[0]);
            }
        }
        let index = titles
            .iter()
            .position(|t| *t == app.table[file].menu[0])
            .unwrap();
        let _ = app.update(Message::MenuToggled(index));
        let _ = app.update(Message::SubmenuToggled(RECENT_SUBMENU));
    }

    #[test]
    fn open_recent_is_a_submenu_of_file_with_the_name_and_the_folder_of_each_file() {
        let _turn = render_turn();
        let (mut app, dir, paths) = app_with_recent("recent-menu", 3);
        open_recent(&mut app);
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(800.0, 560.0),
            app.view(),
        );
        for path in &paths {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            assert!(ui.find(name.as_str()).is_ok(), "{name}");
        }
        assert!(ui.find(dir.display().to_string().as_str()).is_ok());
        assert!(ui.find(app.t("menu-file-recent-clear").as_str()).is_ok());
        // Nothing is missing: no entry to remove what is not there.
        assert!(
            ui.find(app.t("menu-file-recent-remove-missing").as_str())
                .is_err()
        );
        // Choosing an entry opens that file.
        let _ = ui.click("r0.lasession");
        let messages: Vec<Message> = ui.into_messages().collect();
        assert!(
            messages.iter().any(|m| matches!(
                m,
                Message::Run(Command::OpenSession(p)) if *p == paths[0]
            )),
            "{}",
            messages.len()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_file_is_marked_and_cannot_be_opened_and_can_be_removed() {
        let _turn = render_turn();
        let (mut app, dir, paths) = app_with_recent("recent-missing", 3);
        std::fs::remove_file(&paths[1]).unwrap();
        open_recent(&mut app);
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(800.0, 560.0),
            app.view(),
        );
        let marker = format!("r1.lasession  {}", app.t("menu-file-recent-missing"));
        assert!(ui.find(marker.as_str()).is_ok(), "{marker}");
        let _ = ui.click(marker.as_str());
        assert!(
            !ui.into_messages()
                .any(|m| matches!(m, Message::Run(Command::OpenSession(_)))),
            "a missing file has no action"
        );
        assert!(app.shell.recent_files().iter().any(|f| !f.exists));
        let _ = app.update(Message::RemoveMissingRecent);
        let list: Vec<_> = app
            .shell
            .recent_files()
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert_eq!(list, vec![paths[2].clone(), paths[0].clone()]);
        let _ = app.update(Message::Run(Command::ClearRecentFiles));
        assert!(app.shell.recent_files().is_empty());
        // An empty list shows one entry that says so.
        open_recent(&mut app);
        let ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(800.0, 560.0),
            app.view(),
        );
        let mut ui = ui;
        assert!(ui.find(app.t("menu-file-recent-empty").as_str()).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_submenu_of_recent_files_fits_the_minimum_window_in_english_and_japanese() {
        let _turn = render_turn();
        let (mut app, dir, paths) = app_with_recent("recent-fit", 15);
        assert_eq!(app.shell.recent_files().len(), 15);
        std::fs::remove_file(&paths[3]).unwrap();
        for language in [la_i18n::Language::En, la_i18n::Language::Ja] {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            open_recent(&mut app);
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(800.0, 560.0),
                app.view(),
            );
            let found = ui.find(app.t("menu-file-recent-clear").as_str()).unwrap();
            let bounds = found.bounds();
            assert!(
                bounds.x >= 0.0 && bounds.x + bounds.width <= 800.0,
                "{language:?}: {bounds:?}"
            );
            drop(ui);
            let _ = app.update(Message::MenuClosed);
        }
        let _ = std::fs::remove_dir_all(&dir);
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
    fn dragging_a_placed_marker_keeps_the_preview_for_the_snap_ring_until_it_is_released() {
        use la_host::markers::{End, Kind};
        let mut app = app_with_capture("dragging");
        let _ = app.update(Message::Run(Command::BeginPlacement(Kind::Single)));
        let _ = app.update(Message::Run(Command::FixPlacement {
            pointer: pointer_for(&app, 1_000),
        }));
        let id = app.shell.state().markers.list[0].id;
        let pointer = pointer_for(&app, 2_000);
        let _ = app.update(Message::Run(Command::MoveMarkerEnd {
            id,
            end: End::Point,
            pointer,
        }));
        assert_eq!(app.placement_preview.map(|p| p.position), Some(2_000));
        let _ = app.update(Message::Run(Command::EndMarkerDrag));
        assert!(
            app.placement_preview.is_none(),
            "nothing stale after the release"
        );
    }

    #[test]
    fn placing_a_marker_shows_what_to_do_and_follows_the_pointer_until_it_is_fixed() {
        let mut app = app_with_capture("placing");
        let hint = |app: &App| app.shell.status_line().hint;
        assert_eq!(hint(&app), None);
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Pair,
        )));
        assert_eq!(hint(&app), Some(app.t("marker-placing-pair-first")));
        let pointer = pointer_for(&app, 1_000);
        let _ = app.update(Message::PlacementPointer(Some(pointer), Instant::now()));
        assert_eq!(app.placement_preview.map(|p| p.position), Some(1_000));
        let _ = app.update(Message::Run(Command::FixPlacement { pointer }));
        assert_eq!(hint(&app), Some(app.t("marker-placing-pair-second")));
        let _ = app.update(Message::Run(Command::FixPlacement {
            pointer: pointer_for(&app, 5_000),
        }));
        assert_eq!(hint(&app), None);
        assert!(
            app.placement_preview.is_none(),
            "nothing stale is left for the next one"
        );
        assert_eq!(app.shell.state().markers.list.len(), 1);
        // The pointer leaving the view takes the preview away.
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Single,
        )));
        let _ = app.update(Message::PlacementPointer(Some(pointer), Instant::now()));
        let _ = app.update(Message::PlacementPointer(None, Instant::now()));
        assert!(app.placement_preview.is_none());
    }

    #[test]
    fn resting_over_a_crowded_zone_zooms_in_and_the_view_comes_back_when_the_marker_is_fixed() {
        use std::time::Duration;
        let mut app = app_with_capture("dwell");
        let _ = app.update(Message::AreaResized(1000.0));
        let lane = app.plan.channels.first().map(|c| c.channel);
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Single,
        )));
        let t0 = Instant::now();
        let pointer = la_host::markers::Pointer { x: 400.0, lane };
        let target = app
            .shell
            .crowded_zoom_target(pointer)
            .expect("the whole capture is crowded under the pointer");
        let _ = app.update(Message::PlacementPointer(Some(pointer), t0));
        // Under two seconds nothing happens.
        let _ = app.update(Message::DwellTick(t0 + Duration::from_millis(1_900)));
        assert_eq!(app.shell.state().snap_zoom, None);
        assert!(app.shell.state().auto_fit);
        // At two seconds the zoom starts; 0.4 s later it is at the target.
        let _ = app.update(Message::DwellTick(t0 + Duration::from_secs(2)));
        assert!(app.shell.state().snap_zoom.is_some());
        assert_eq!(
            app.shell.status_line().hint,
            Some(app.t("marker-zoomed-in"))
        );
        let _ = app.update(Message::DwellTick(t0 + Duration::from_millis(2_200)));
        let middle = app.shell.state().window;
        assert!(middle.length < target.length * 100, "{middle:?} on its way");
        let _ = app.update(Message::DwellTick(t0 + Duration::from_millis(2_450)));
        let state = app.shell.state();
        assert_eq!(
            (state.window.start, state.window.length),
            (target.start, target.length)
        );
        // A click fixes the marker and brings the whole capture back.
        let click = pointer_for(&app, target.start + 3);
        let _ = app.update(Message::Run(Command::FixPlacement {
            pointer: la_host::markers::Pointer { lane, ..click },
        }));
        let state = app.shell.state();
        assert!(state.auto_fit);
        assert_eq!(state.snap_zoom, None);
        assert_eq!(state.markers.list.len(), 1);
        assert_eq!(app.shell.status_line().hint, None);
        assert!(app.zoom_animation.is_none());
    }

    #[test]
    fn moving_the_pointer_or_cancelling_stops_the_wait_for_the_zoom() {
        use std::time::Duration;
        let mut app = app_with_capture("dwell-stop");
        let _ = app.update(Message::AreaResized(1000.0));
        let lane = app.plan.channels.first().map(|c| c.channel);
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Single,
        )));
        let t0 = Instant::now();
        let at = |x: f32| la_host::markers::Pointer { x, lane };
        let _ = app.update(Message::PlacementPointer(Some(at(400.0)), t0));
        // Moved by three pixels at 1.9 s: the two seconds start again.
        let _ = app.update(Message::PlacementPointer(
            Some(at(403.0)),
            t0 + Duration::from_millis(1_900),
        ));
        let _ = app.update(Message::DwellTick(t0 + Duration::from_millis(2_100)));
        assert_eq!(app.shell.state().snap_zoom, None);
        // Cancelled at 3 s: the clock is forgotten, whatever time it is.
        let _ = app.update(Message::Run(Command::CancelPlacement));
        let _ = app.update(Message::DwellTick(t0 + Duration::from_secs(30)));
        assert_eq!(app.shell.state().snap_zoom, None);
        assert!(app.shell.state().auto_fit);
    }

    /// The Markers view alone, at the narrowest width the side panel can have.
    fn markers_view_of(app: &App) -> iced_test::Simulator<'_, Message> {
        iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(220.0, 600.0),
            app.markers_panel(),
        )
    }

    #[test]
    fn the_markers_view_has_the_two_buttons_and_a_row_for_each_marker() {
        let _turn = render_turn();
        let mut app = app_with_capture("marker-view");
        let _ = app.update(Message::AreaResized(1000.0));
        for language in la_i18n::Language::ALL {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            let (single, pair, empty) = (
                app.t("marker-single-add"),
                app.t("marker-pair-add"),
                app.t("marker-empty"),
            );
            let mut ui = markers_view_of(&app);
            assert!(ui.find(single.as_str()).is_ok(), "{language:?}: {single}");
            assert!(ui.find(pair.as_str()).is_ok(), "{language:?}: {pair}");
            assert!(ui.find(empty.as_str()).is_ok(), "{language:?}: {empty}");
        }
        let _ = app.update(Message::Run(Command::SetLanguage(la_i18n::Language::En)));
        place_single(&mut app, 3_000);
        place_pair(&mut app, 1_000, 2_000);
        place_single(&mut app, 500);
        let mut ui = markers_view_of(&app);
        for name in ["T1", "P1", "T2"] {
            assert!(ui.find(name).is_ok(), "{name}");
        }
        assert!(ui.find(app.t("marker-empty").as_str()).is_err());
        // The rows follow the time of the (start) position, not the order of creation.
        let order: Vec<String> = app
            .shell
            .state()
            .markers
            .ordered()
            .iter()
            .map(|m| m.name())
            .collect();
        assert_eq!(order, ["T2", "P1", "T1"]);
    }

    #[test]
    fn the_button_of_the_kind_being_placed_is_pressed_and_pressing_it_again_cancels() {
        let mut app = app_with_capture("marker-buttons");
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Pair,
        )));
        assert_eq!(
            app.shell.state().placement.map(|p| p.kind),
            Some(la_host::markers::Kind::Pair)
        );
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Pair,
        )));
        assert_eq!(app.shell.state().placement, None);
    }

    #[test]
    fn the_actions_of_a_marker_open_and_close_and_the_bin_deletes_without_asking() {
        let _turn = render_turn();
        let mut app = app_with_capture("marker-actions");
        let _ = app.update(Message::AreaResized(1000.0));
        place_single(&mut app, 3_000);
        let id = app.shell.state().markers.list[0].id;
        let delete = app.t("marker-delete");
        let comment = app.t("marker-comment");
        assert!(markers_view_of(&app).find(delete.as_str()).is_err());
        let _ = app.update(Message::MarkerMenu(id));
        {
            let mut ui = markers_view_of(&app);
            assert!(ui.find(delete.as_str()).is_ok(), "the list is open");
            assert!(ui.find(comment.as_str()).is_ok(), "the comment field");
        }
        // Pressing again closes it, and so does Escape.
        let _ = app.update(Message::MarkerMenu(id));
        assert!(markers_view_of(&app).find(delete.as_str()).is_err());
        let _ = app.update(Message::MarkerMenu(id));
        let escape = keyboard::Key::Named(keyboard::key::Named::Escape);
        let _ = app.update(Message::Key(keyboard::Event::KeyPressed {
            key: escape.clone(),
            modified_key: escape,
            physical_key: keyboard::key::Physical::Code(keyboard::key::Code::Escape),
            location: keyboard::Location::Standard,
            modifiers: keyboard::Modifiers::empty(),
            text: None,
            repeat: false,
        }));
        assert!(markers_view_of(&app).find(delete.as_str()).is_err());
        // A right click on the mark in the view selects it and shows its actions.
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Capture));
        let _ = app.update(Message::MarkerContext(id));
        assert_eq!(app.shell.state().side_panel, la_host::SidePanel::Markers);
        assert_eq!(app.shell.state().markers.selected, Some(id));
        assert!(markers_view_of(&app).find(delete.as_str()).is_ok());
        // The delete button of the list removes it at once, and nothing stale is left.
        let _ = app.update(Message::Run(Command::RemoveMarker(id)));
        assert!(app.shell.state().markers.list.is_empty());
        assert_eq!(app.marker_menu, None);
        assert!(app.marker_comments.is_empty());
    }

    #[test]
    fn a_comment_typed_in_the_list_is_kept_by_the_host_and_cut_to_its_limit() {
        let mut app = app_with_capture("marker-comment");
        let _ = app.update(Message::AreaResized(1000.0));
        place_single(&mut app, 3_000);
        let id = app.shell.state().markers.list[0].id;
        let _ = app.update(Message::MarkerComment(id, "rising edge".into()));
        assert_eq!(app.shell.state().markers.list[0].comment, "rising edge");
        assert_eq!(
            app.marker_comments.get(&id).map(String::as_str),
            Some("rising edge")
        );
        let long = "x".repeat(200);
        let _ = app.update(Message::MarkerComment(id, long));
        assert_eq!(
            app.shell.state().markers.list[0].comment.chars().count(),
            80
        );
    }

    #[test]
    fn at_the_limit_the_view_says_why_nothing_can_be_added() {
        let _turn = render_turn();
        let mut app = app_with_capture("marker-limit");
        let _ = app.update(Message::AreaResized(1000.0));
        for i in 0..64 {
            place_single(&mut app, 1_000 + i * 100);
        }
        assert_eq!(app.shell.state().markers.list.len(), 64);
        let limit = app.t("marker-limit");
        assert!(markers_view_of(&app).find(limit.as_str()).is_ok());
        // The commands are refused too.
        let _ = app.update(Message::Run(Command::BeginPlacement(
            la_host::markers::Kind::Single,
        )));
        assert_eq!(app.shell.state().placement, None);
    }

    #[test]
    fn two_checked_single_markers_can_be_made_a_pair() {
        let _turn = render_turn();
        let mut app = app_with_capture("marker-make-pair");
        let _ = app.update(Message::AreaResized(1000.0));
        place_single(&mut app, 1_000);
        place_single(&mut app, 2_000);
        place_single(&mut app, 3_000);
        let make = app.t("marker-make-pair");
        let ids: Vec<u32> = app
            .shell
            .state()
            .markers
            .list
            .iter()
            .map(|m| m.id)
            .collect();
        let _ = app.update(Message::MarkerCheck(ids[0], true));
        assert!(
            markers_view_of(&app).find(make.as_str()).is_err(),
            "one is not enough"
        );
        let _ = app.update(Message::MarkerCheck(ids[1], true));
        assert!(markers_view_of(&app).find(make.as_str()).is_ok());
        let _ = app.update(Message::MarkerCheck(ids[2], true));
        assert!(
            markers_view_of(&app).find(make.as_str()).is_err(),
            "three are too many"
        );
        let _ = app.update(Message::MarkerCheck(ids[2], false));
        let _ = app.update(Message::Run(Command::MakePair {
            first: ids[0],
            second: ids[1],
        }));
        assert_eq!(app.shell.state().markers.list.len(), 2);
        assert!(
            app.marker_checked.is_empty(),
            "the checks of markers that are gone go away"
        );
        assert!(markers_view_of(&app).find(make.as_str()).is_err());
    }

    #[test]
    fn the_actions_of_a_single_marker_and_of_a_pair_are_those_of_their_kind() {
        let _turn = render_turn();
        let mut app = app_with_capture("marker-kinds");
        place_single(&mut app, 100_000);
        place_pair(&mut app, 150_000, 200_000);
        let single = app.shell.state().markers.list[0].id;
        let pair = app.shell.state().markers.list[1].id;
        let singles = [
            "marker-action-origin",
            "marker-action-cut-left",
            "marker-action-cut-right",
            "marker-action-scroll",
        ];
        let pairs = [
            "marker-action-trim",
            "marker-action-fit",
            "marker-action-split",
        ];
        for language in la_i18n::Language::ALL {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            let _ = app.update(Message::MarkerMenu(single));
            {
                let mut ui = markers_view_of(&app);
                for key in singles {
                    assert!(ui.find(app.t(key).as_str()).is_ok(), "{language:?} {key}");
                }
                for key in pairs {
                    assert!(ui.find(app.t(key).as_str()).is_err(), "{language:?} {key}");
                }
            }
            let _ = app.update(Message::MarkerMenu(pair));
            {
                let mut ui = markers_view_of(&app);
                for key in pairs {
                    assert!(ui.find(app.t(key).as_str()).is_ok(), "{language:?} {key}");
                }
                for key in singles {
                    assert!(ui.find(app.t(key).as_str()).is_err(), "{language:?} {key}");
                }
            }
            let _ = app.update(Message::MarkerMenu(pair));
        }
    }

    #[test]
    fn the_marker_that_time_counts_from_offers_to_count_from_the_start_again() {
        let _turn = render_turn();
        let mut app = app_with_capture("marker-origin");
        place_single(&mut app, 100_000);
        let id = app.shell.state().markers.list[0].id;
        let _ = app.update(Message::MarkerMenu(id));
        let (origin, reset) = (
            app.t("marker-action-origin"),
            app.t("marker-action-origin-reset"),
        );
        {
            let mut ui = markers_view_of(&app);
            assert!(ui.find(origin.as_str()).is_ok());
            assert!(ui.find(reset.as_str()).is_err());
        }
        let _ = app.update(Message::Run(Command::SetTimeOrigin(id)));
        {
            let mut ui = markers_view_of(&app);
            assert!(ui.find(reset.as_str()).is_ok());
            assert!(ui.find(origin.as_str()).is_err());
        }
        // The axis counts from the marker: its labels are negative before it.
        let first = app.plan.axis.ticks.first().expect("ticks");
        assert!(first.seconds < 0.0, "{first:?}");
        assert!(app.plan.axis.ticks.iter().any(|t| t.seconds == 0.0));
    }

    #[test]
    fn a_cut_asks_first_names_what_is_lost_and_shows_its_progress_and_cancel() {
        let _turn = render_turn();
        let mut app = app_with_capture("marker-cut");
        place_single(&mut app, 150_000);
        let id = app.shell.state().markers.list[0].id;
        let _ = app.update(Message::Run(Command::CutLeft(id)));
        assert!(matches!(
            app.shell.state().pending_decision,
            Some(la_host::Guarded::Crop(_))
        ));
        for language in la_i18n::Language::ALL {
            let _ = app.update(Message::Run(Command::SetLanguage(language)));
            let size = iced::Size::new(800.0, 560.0);
            let mut ui =
                iced_test::Simulator::with_size(crate::fonts::settings(), size, app.view());
            for key in ["dialog-crop-title", "dialog-crop-confirm", "dialog-cancel"] {
                assert!(ui.find(app.t(key).as_str()).is_ok(), "{language:?} {key}");
            }
        }
        let _ = app.update(Message::Run(Command::SetLanguage(la_i18n::Language::En)));
        // Cancel keeps everything; delete starts the cut, which shows its progress and a cancel.
        let _ = app.update(Message::Run(Command::Decide(Decision::Cancel)));
        assert_eq!(app.shell.state().pending_decision, None);
        let _ = app.update(Message::Run(Command::CutLeft(id)));
        let _ = app.update(Message::Run(Command::Decide(Decision::Discard)));
        assert_eq!(app.shell.state().activity, Activity::Cropping);
        {
            let mut ui = iced_test::Simulator::with_size(
                crate::fonts::settings(),
                iced::Size::new(800.0, 560.0),
                app.view(),
            );
            assert!(ui.find(app.t("dialog-cropping").as_str()).is_ok());
            assert!(ui.find(app.t("dialog-cancel").as_str()).is_ok());
        }
        let deadline = Instant::now().checked_add(Duration::from_secs(30)).unwrap();
        while app.shell.state().activity != Activity::Idle {
            let _ = app.update(Message::Tick(Instant::now()));
            assert!(Instant::now() < deadline, "the cut did not end");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(
            app.shell
                .state()
                .capture
                .as_ref()
                .unwrap()
                .snapshot
                .length(),
            150_000
        );
    }

    #[test]
    fn a_pair_marker_shows_its_duration_and_frequency() {
        let _turn = render_turn();
        let mut app = app_with_capture("markers");
        let _ = app.update(Message::Run(Command::SetSnap(false)));
        let _ = app.update(Message::Run(Command::ZoomToRange {
            start: 150_000,
            end: 150_400,
        }));
        let _ = app.update(Message::AreaResized(1000.0));
        place_pair(&mut app, 150_080, 150_280);
        // The measurement is in the Markers view of the side panel.
        let _ = app.update(Message::SidePanel(la_host::SidePanel::Markers));
        assert_eq!(app.plan.markers.len(), 1);
        let texts = app.shell.marker_texts();
        let duration = texts[0].duration.clone().unwrap();
        assert!(duration.starts_with("200"), "{duration}");
        assert!(texts[0].frequency.is_some());
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        assert!(ui.find("P1").is_ok());
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
        place_single(&mut app, 100_000);
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
        let _ = app.update(Message::Run(Command::RenameChannel {
            channel: 4,
            name: "SDA".into(),
        }));
        let _ = app.update(Message::Run(Command::SetChannelColour {
            channel: 4,
            colour: la_host::presentation::Colour::Palette(5),
        }));
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
            assert_eq!(style.colour, la_host::presentation::Colour::Palette(5));
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

        // The Capture view before a capture lists the channels as buttons.
        let mut ui = iced_test::Simulator::with_size(
            crate::fonts::settings(),
            iced::Size::new(1180.0, 720.0),
            app.view(),
        );
        assert!(ui.find("31").is_ok(), "the button of channel 31");
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
        let _ = app.update(Message::Run(Command::RenameChannel {
            channel: 2,
            name: "SCL".into(),
        }));
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
