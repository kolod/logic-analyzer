//! The application: state, messages, update and view.
//!
//! Everything the application does is a command of the host; this module maps input to commands
//! and the host state to widgets. It contains no domain logic.

use std::fmt;
use std::time::{Duration, Instant};

use iced::widget::{
    Space, button, canvas, checkbox, column, container, mouse_area, pick_list, row, scrollable,
    stack, text, text_input, tooltip,
};
use iced::{Element, Fill, Length, Subscription, Task, Theme, keyboard, theme, window};
use la_devices::DeviceId;
use la_host::presentation::{Colour, PALETTE_SIZE};
use la_host::shortcuts::{Accelerator, Action, ShortcutEntry};
use la_host::window::ViewSize;
use la_host::{
    Activity, Changes, Command, ConfigEdit, Decision, Outcome, Shell, ShellConfig, SignalProfile,
    Tick,
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
    /// Run a host command.
    Run(Command),
    /// A menu title was clicked.
    MenuToggled(usize),
    /// The open menu is dismissed.
    MenuClosed,
    /// A device was chosen.
    DevicePicked(DeviceChoice),
    /// A number of channels was chosen.
    ChannelsPicked(ChannelCount),
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
    /// A marker shortcut or menu entry: place, previous, next, remove selected, snap.
    MarkerAction(MarkerAction),
    /// A marker was renamed in the markers panel.
    MarkerName(u32, String),
    /// A marker becomes the first (0) or second (1) of the measurement.
    MarkerMeasure(usize, u32),
    /// Nothing to do.
    Noop,
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

/// A number of channels in the chooser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelCount(u8);

impl fmt::Display for ChannelCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
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
        // The plus key is a shifted `=` on many layouts: accept both for zooming in.
        keyboard::Key::Character(text) if text.as_str() == "=" => Some("+".to_owned()),
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
    table.iter().find(|entry| {
        entry.accelerator.is_some_and(|a| {
            a.key == name
                && a.ctrl == pressed.ctrl
                && a.shift == pressed.shift
                && a.alt == pressed.alt
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
    plan_key: (u64, u64, u64, u64, u32, bool),
    cache: canvas::Cache,
    cache_key: (u64, Variant, la_i18n::Language),
    open_menu: Option<usize>,
    rate_text: String,
    length_text: String,
    seed_text: String,
    names: std::collections::BTreeMap<u8, String>,
    marker_names: std::collections::BTreeMap<u32, String>,
    messages_open: bool,
}

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
            plan_key: (0, 0, 0, 0, 0, false),
            cache: canvas::Cache::new(),
            cache_key: (0, variant, la_i18n::Language::En),
            open_menu: None,
            rate_text: String::new(),
            length_text: String::new(),
            seed_text: String::new(),
            names: std::collections::BTreeMap::new(),
            marker_names: std::collections::BTreeMap::new(),
            messages_open: false,
        };
        app.sync_texts();
        app.sync_names();
        app.refresh();
        app
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
        self.t("app-title")
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
            Outcome::Done(changes) if changes.contains(Changes::QUIT) => iced::exit(),
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
            Message::Key(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                let action = entry_for(&self.table, &key, modifiers)
                    .filter(|entry| entry.enabled_when.is_enabled(self.shell.state()))
                    .map(|entry| entry.action.clone());
                match action {
                    Some(Action::Command(command)) => self.run(command),
                    Some(Action::ToggleFollowLatest) => self.update(Message::ToggleFollow),
                    Some(Action::ShowLog) => self.update(Message::ShowLog),
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
                    Some(Action::ToggleSnap) => {
                        self.update(Message::MarkerAction(MarkerAction::ToggleSnap))
                    }
                    _ => Task::none(),
                }
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
            Message::ChannelsPicked(count) => {
                let mask = 1_u32
                    .checked_shl(u32::from(count.0))
                    .map_or(u32::MAX, |bit| bit.wrapping_sub(1));
                self.run(Command::EditConfig(ConfigEdit::Channels(mask)))
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
                    _ => {}
                }
            }
            items = items.push(item);
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

    fn toolbar(&self) -> Element<'_, Message> {
        let state = self.shell.state();
        let idle = state.activity == Activity::Idle;
        let localizer = self.shell.localizer();
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
        let channel_limit = state
            .selected_device
            .as_ref()
            .and_then(|id| state.devices.iter().find(|d| d.id == *id))
            .map_or(8, |d| d.capabilities.channels);
        let counts: Vec<ChannelCount> = [1_u8, 2, 4, 8, 16, 32]
            .into_iter()
            .filter(|n| *n <= channel_limit)
            .map(ChannelCount)
            .collect();
        let mask = state.config.enabled_channels;
        let chosen = u8::try_from(mask.count_ones())
            .ok()
            .filter(|n| counts.iter().any(|c| c.0 == *n))
            .map(ChannelCount);
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
        let mut channel_picker = pick_list(counts, chosen, Message::ChannelsPicked);
        let mut profile_picker = pick_list(profiles, profile, Message::ProfilePicked);
        let mut rate = text_input("", &self.rate_text).width(110);
        let mut length = text_input("", &self.length_text).width(110);
        let mut seed = text_input(&self.t("toolbar-seed-random"), &self.seed_text).width(90);
        let mut realtime = checkbox(state.config.realtime).label(self.t("toolbar-realtime"));
        if idle {
            device_picker = device_picker.on_open(Message::Noop);
            rate = rate.on_input(Message::RateText);
            length = length.on_input(Message::LengthText);
            seed = seed.on_input(Message::SeedText);
            realtime = realtime.on_toggle(Message::RealtimeToggled);
        }
        let _ = (&mut channel_picker, &mut profile_picker);

        let start = if idle {
            let mut b = button(text(self.t("toolbar-start")));
            if state.config_problems.is_empty() && state.selected_device.is_some() {
                b = b.on_press(Message::Run(Command::StartCapture));
            }
            b
        } else {
            button(text(self.t("toolbar-stop"))).on_press(Message::Run(Command::StopCapture))
        };

        let controls = row![
            labelled(self.t("toolbar-device"), device_picker.into()),
            labelled(self.t("toolbar-channels"), channel_picker.into()),
            labelled(self.t("toolbar-rate"), rate.into()),
            labelled(self.t("toolbar-length"), length.into()),
            labelled(self.t("toolbar-seed"), seed.into()),
            labelled(self.t("toolbar-profile"), profile_picker.into()),
            realtime,
            start,
        ]
        .spacing(12)
        .align_y(iced::Alignment::End);

        let mut toolbar = column![container(controls).padding([6, 8])];
        for problem in &state.config_problems {
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
            .height(Length::FillPortion(2))
            .style(container::bordered_box)
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
                tooltip(
                    checkbox(style.visible).on_toggle(move |v| Message::ChannelVisible(index, v)),
                    text(self.t("channel-show")),
                    tooltip::Position::Bottom,
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

    fn unsaved_dialog(&self) -> Element<'_, Message> {
        let body = column![
            text(self.t("dialog-unsaved-title")).size(18),
            text(self.t("dialog-unsaved-body")),
            row![
                button(text(self.t("dialog-save")))
                    .on_press(Message::Run(Command::Decide(Decision::Save))),
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
        })
        .width(Fill)
        .height(Fill);

        let mut content = column![self.menu_bar(), self.toolbar()];
        if let Some(banner) = self.banner() {
            content = content.push(banner);
        }
        let content = content
            .push(
                row![
                    signal,
                    column![self.markers_panel(), self.channel_panel()].width(250)
                ]
                .height(Fill),
            )
            .push(self.status_bar());

        let mut layers: Vec<Element<'_, Message>> = vec![content.into()];
        if let Some(menu) = self.open_menu_view() {
            layers.push(
                mouse_area(Space::new().width(Fill).height(Fill))
                    .on_press(Message::MenuClosed)
                    .into(),
            );
            layers.push(menu);
        }
        if self.shell.state().pending_decision.is_some() {
            layers.push(
                mouse_area(container(Space::new().width(Fill).height(Fill)).style(container::dark))
                    .into(),
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
        let app = app_with_capture("render");
        assert!(!app.plan.channels.is_empty());
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
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
    fn the_view_renders_a_zoomed_capture_with_single_edges() {
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
            iced::Settings::default(),
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
            iced::Settings::default(),
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
        assert_eq!(app.plan.markers.len(), 2);
        let text = app.shell.measurement_text().unwrap();
        assert!(text.distance.starts_with("200"), "{}", text.distance);
        assert!(text.frequency.is_some());
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
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
    fn accelerators_are_described_for_the_menu() {
        assert_eq!(describe(Accelerator::ctrl_shift("S")), "Ctrl+Shift+S");
        assert_eq!(describe(Accelerator::key("F5")), "F5");
    }
}
