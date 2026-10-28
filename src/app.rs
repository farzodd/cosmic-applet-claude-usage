use std::time::{Duration, Instant};

use chrono::{DateTime, Local, Utc};
use cosmic::app::{Core, Task};
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::platform_specific::shell::commands::popup::{destroy_popup, get_popup};
use cosmic::iced::window::Id;
use cosmic::iced::{Alignment, Color, Length, Subscription};
use cosmic::widget::{self, svg};
use cosmic::{Element, applet};

use crate::api::{self, Limit, Usage};
use crate::chart;
use crate::config::{Config, DisplayMode};

pub const APP_ID: &str = "io.github.farzodd.CosmicAppletClaudeUsage";
const CLAUDE_SVG: &[u8] = include_bytes!("../res/claude.svg");
const SESSION_WINDOW: chrono::Duration = chrono::Duration::hours(5);
const WEEKLY_WINDOW: chrono::Duration = chrono::Duration::days(7);

pub struct Applet {
    core: Core,
    popup: Option<Id>,
    config: Config,
    config_handler: Option<cosmic_config::Config>,
    usage: Option<Usage>,
    error: Option<api::Error>,
    last_success: Option<DateTime<Utc>>,
    fetching: bool,
    next_fetch: Instant,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    PopupClosed(Id),
    Tick,
    Refresh,
    Fetched(Result<Usage, api::Error>),
    ConfigChanged(Config),
    SetDisplay(DisplayMode),
    SetLabels(bool),
    SetPace(bool),
}

impl Applet {
    fn fetch(&mut self) -> Task<Message> {
        if self.fetching {
            return Task::none();
        }
        self.fetching = true;
        cosmic::task::future(async { Message::Fetched(api::fetch().await) })
    }

    fn save_config(&self) {
        if let Some(handler) = &self.config_handler {
            if let Err(e) = self.config.write_entry(handler) {
                eprintln!("failed to save config: {e}");
            }
        }
    }

    /// Color for the pace tick: contrasts with the panel background.
    fn tick_color(&self) -> &'static str {
        let theme = self.core.applet.theme().unwrap_or_else(cosmic::theme::active);
        if theme.cosmic().is_dark { "#FFFFFF" } else { "#000000" }
    }

    /// Side length for the Claude mark and charts. Uses the panel's full-color icon
    /// size (24px on XS), which is what minimon's graphs are drawn at; the symbolic
    /// size is only 16px and looks undersized next to them.
    fn graphic_size(&self) -> f32 {
        f32::from(self.core.applet.suggested_size(false).0)
    }

    fn pace(&self, limit: &Limit, window: chrono::Duration, now: DateTime<Utc>) -> Option<f32> {
        if !self.config.show_pace {
            return None;
        }
        let reset = limit.resets_at.filter(|r| *r > now)?;
        let remaining = (reset - now).num_seconds() as f32;
        Some(1.0 - remaining / window.num_seconds() as f32)
    }

    fn panel_reading<'a>(
        &self,
        label: &'static str,
        limit: Option<&Limit>,
        window: chrono::Duration,
        now: DateTime<Utc>,
    ) -> Element<'a, Message> {
        let size = self.graphic_size();
        let horizontal = self.core.applet.is_horizontal();
        let percent = limit.map(|l| l.current(now));
        let mut items: Vec<Element<'a, Message>> = Vec::new();

        if self.config.show_labels {
            items.push(
                self.core
                    .applet
                    .text(label)
                    .class(cosmic::theme::Text::Custom(dim_text))
                    .into(),
            );
        }
        if self.config.display.chart() {
            let pace = limit.and_then(|l| self.pace(l, window, now));
            items.push(
                svg(chart::square(percent.unwrap_or(0.0), pace, self.tick_color()))
                    .width(Length::Fixed(size))
                    .height(Length::Fixed(size))
                    .into(),
            );
        }
        if self.config.display.value() {
            let value = match percent {
                Some(p) if horizontal => format!("{p:.0}%"),
                Some(p) => format!("{p:.0}"),
                None => "–".to_owned(),
            };
            items.push(self.core.applet.text(value).into());
        }

        if horizontal {
            widget::row::with_children(items)
                .spacing(4)
                .align_y(Alignment::Center)
                .into()
        } else {
            widget::column::with_children(items)
                .spacing(2)
                .align_x(Alignment::Center)
                .into()
        }
    }

    fn popup_section<'a>(
        &self,
        title: String,
        limit: &Limit,
        window: chrono::Duration,
        now: DateTime<Utc>,
    ) -> Element<'a, Message> {
        let percent = limit.current(now);
        let pace = self.pace(limit, window, now);
        let reset = match limit.resets_at {
            Some(r) if r > now => {
                let remaining = fmt_duration(r - now);
                if window > chrono::Duration::days(1) {
                    let local = r.with_timezone(&Local).format("%a %-I:%M %p");
                    format!("Resets {local} (in {remaining})")
                } else {
                    format!("Resets in {remaining}")
                }
            }
            Some(_) => "Window has reset".to_owned(),
            None => "Not started".to_owned(),
        };
        let mut caption = reset;
        if let Some(p) = pace {
            caption.push_str(&format!(" · {:.0}% of window elapsed", p * 100.0));
        }

        widget::column::with_children(vec![
            widget::row::with_children(vec![
                widget::text::body(title).into(),
                widget::space::horizontal().into(),
                widget::text::heading(format!("{percent:.0}%")).into(),
            ])
            .align_y(Alignment::Center)
            .into(),
            svg(chart::bar(percent, pace, self.tick_color()))
                .width(Length::Fill)
                .height(Length::Fixed(10.0))
                .into(),
            widget::text::caption(caption)
                .class(cosmic::theme::Text::Custom(dim_text))
                .into(),
        ])
        .spacing(6)
        .into()
    }

    fn status_line(&self, now: DateTime<Utc>) -> String {
        if self.fetching && self.usage.is_none() {
            return "Loading…".to_owned();
        }
        let updated = self.last_success.map(|t| {
            let ago = now - t;
            if ago < chrono::Duration::minutes(1) {
                "Updated just now".to_owned()
            } else {
                format!("Updated {} ago", fmt_duration(ago))
            }
        });
        match (&self.error, updated) {
            (Some(e), Some(u)) => format!("{e}\n{u}"),
            (Some(e), None) => e.to_string(),
            (None, Some(u)) => u,
            (None, None) => String::new(),
        }
    }
}

fn dim_text(theme: &cosmic::Theme) -> cosmic::iced::widget::text::Style {
    let mut c: Color = theme.cosmic().on_bg_color().into();
    c.a = 0.65;
    cosmic::iced::widget::text::Style {
        color: Some(c),
        ..Default::default()
    }
}

fn fmt_duration(d: chrono::Duration) -> String {
    let mins = d.num_minutes().max(0);
    let (days, hours, m) = (mins / 1440, (mins / 60) % 24, mins % 60);
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {m}m")
    } else {
        format!("{m}m")
    }
}

impl cosmic::Application for Applet {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: ()) -> (Self, Task<Message>) {
        let config_handler = cosmic_config::Config::new(APP_ID, Config::VERSION).ok();
        let config = config_handler
            .as_ref()
            .map(|h| Config::get_entry(h).unwrap_or_else(|(_, c)| c))
            .unwrap_or_default();
        let mut applet = Applet {
            core,
            popup: None,
            config,
            config_handler,
            usage: None,
            error: None,
            last_success: None,
            fetching: false,
            next_fetch: Instant::now(),
        };
        let task = applet.fetch();
        (applet, task)
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            cosmic::iced::time::every(Duration::from_secs(30)).map(|_| Message::Tick),
            self.core
                .watch_config::<Config>(APP_ID)
                .map(|update| Message::ConfigChanged(update.config)),
        ])
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TogglePopup => {
                if let Some(id) = self.popup.take() {
                    return destroy_popup(id);
                }
                let id = Id::unique();
                self.popup = Some(id);
                let settings = self.core.applet.get_popup_settings(
                    self.core.main_window_id().unwrap(),
                    id,
                    None,
                    None,
                    None,
                );
                let stale = self
                    .last_success
                    .is_none_or(|t| Utc::now() - t > chrono::Duration::minutes(1));
                let refresh = if stale { self.fetch() } else { Task::none() };
                return Task::batch([get_popup(settings), refresh]);
            }
            Message::PopupClosed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                }
            }
            Message::Tick => {
                if Instant::now() >= self.next_fetch {
                    return self.fetch();
                }
            }
            Message::Refresh => return self.fetch(),
            Message::Fetched(result) => {
                self.fetching = false;
                let retry = match result {
                    Ok(usage) => {
                        self.usage = Some(usage);
                        self.error = None;
                        self.last_success = Some(Utc::now());
                        self.config.refresh_secs.max(60)
                    }
                    Err(e) => {
                        eprintln!("usage fetch failed: {e}");
                        let retry = match e {
                            // Cheap file check; pick up a fresh login quickly.
                            api::Error::NotLoggedIn => 30,
                            api::Error::RateLimited => 900,
                            api::Error::LoginExpired => 300,
                            api::Error::Other(_) => 60,
                        };
                        self.error = Some(e);
                        retry
                    }
                };
                self.next_fetch = Instant::now() + Duration::from_secs(retry);
            }
            Message::ConfigChanged(config) => self.config = config,
            Message::SetDisplay(mode) => {
                self.config.display = mode;
                self.save_config();
            }
            Message::SetLabels(on) => {
                self.config.show_labels = on;
                self.save_config();
            }
            Message::SetPace(on) => {
                self.config.show_pace = on;
                self.save_config();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let now = Utc::now();
        let horizontal = self.core.applet.is_horizontal();
        let (size, _) = self.core.applet.suggested_size(true);
        let (major_pad, minor_pad) = self.core.applet.suggested_padding(true);

        let icon = svg(svg::Handle::from_memory(CLAUDE_SVG))
            .width(Length::Fixed(self.graphic_size()))
            .height(Length::Fixed(self.graphic_size()));
        let usage = self.usage.as_ref();
        let children: Vec<Element<'_, Message>> = vec![
            icon.into(),
            self.panel_reading("S", usage.and_then(|u| u.session.as_ref()), SESSION_WINDOW, now),
            self.panel_reading("W", usage.and_then(|u| u.weekly.as_ref()), WEEKLY_WINDOW, now),
        ];

        let content: Element<'_, Message> = if horizontal {
            widget::row::with_children(children)
                .spacing(8)
                .align_y(Alignment::Center)
                .into()
        } else {
            widget::column::with_children(children)
                .spacing(6)
                .align_x(Alignment::Center)
                .into()
        };

        let (padding, content) = if horizontal {
            let c = widget::container(content)
                .center_y(Length::Fixed(f32::from(size + 2 * minor_pad)));
            ([0, major_pad], c)
        } else {
            let c = widget::container(content)
                .center_x(Length::Fixed(f32::from(size + 2 * minor_pad)));
            ([major_pad, 0], c)
        };
        let button = widget::button::custom(content)
            .padding(padding)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_down(Message::TogglePopup);

        self.core.applet.autosize_window(button).into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Message> {
        let now = Utc::now();
        let mut col: Vec<Element<'_, Message>> = Vec::new();

        col.push(
            widget::row::with_children(vec![
                svg(svg::Handle::from_memory(CLAUDE_SVG))
                    .width(Length::Fixed(20.0))
                    .height(Length::Fixed(20.0))
                    .into(),
                widget::text::heading("Claude usage").into(),
                widget::space::horizontal().into(),
                widget::button::icon(widget::icon::from_name("view-refresh-symbolic"))
                    .on_press(Message::Refresh)
                    .into(),
            ])
            .spacing(8)
            .align_y(Alignment::Center)
            .into(),
        );

        if let Some(usage) = &self.usage {
            if let Some(l) = &usage.session {
                col.push(self.popup_section("Current session".into(), l, SESSION_WINDOW, now));
            }
            if let Some(l) = &usage.weekly {
                col.push(self.popup_section("Current week".into(), l, WEEKLY_WINDOW, now));
            }
            for (label, l) in &usage.extra {
                col.push(self.popup_section(label.clone(), l, WEEKLY_WINDOW, now));
            }
        }

        let status = self.status_line(now);
        if !status.is_empty() {
            col.push(
                widget::text::caption(status)
                    .class(cosmic::theme::Text::Custom(dim_text))
                    .into(),
            );
        }

        col.push(widget::divider::horizontal::default().into());

        let mode_buttons: Vec<Element<'_, Message>> = DisplayMode::ALL
            .into_iter()
            .map(|mode| {
                let b = widget::button::text(mode.label()).on_press(Message::SetDisplay(mode));
                if mode == self.config.display {
                    b.class(cosmic::theme::Button::Suggested).into()
                } else {
                    b.into()
                }
            })
            .collect();
        col.push(
            widget::row::with_children(vec![
                widget::text::body("Panel shows").into(),
                widget::space::horizontal().into(),
                widget::row::with_children(mode_buttons).spacing(4).into(),
            ])
            .align_y(Alignment::Center)
            .into(),
        );
        col.push(
            widget::settings::item(
                "Labels (S / W)",
                widget::toggler(self.config.show_labels).on_toggle(Message::SetLabels),
            )
            .into(),
        );
        col.push(
            widget::settings::item(
                "Pace marker",
                widget::toggler(self.config.show_pace).on_toggle(Message::SetPace),
            )
            .into(),
        );

        let content = widget::column::with_children(col).spacing(14).padding(16);
        self.core.applet.popup_container(content).into()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(applet::style())
    }
}
