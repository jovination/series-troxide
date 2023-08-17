use std::sync::mpsc;

use crate::core::api::series_information::SeriesMainInformation;
use crate::core::api::updates::show_updates::*;
use crate::core::caching::tv_schedule::{get_series_with_country, get_series_with_date};
use crate::core::settings_config::locale_settings;
use crate::gui::assets::icons::BINOCULARS_FILL;
use crate::gui::series_page;
use crate::gui::troxide_widget;
use crate::gui::troxide_widget::series_poster::{Message as SeriesPosterMessage, SeriesPoster};
use searching::Message as SearchMessage;

use iced::widget::{column, container, scrollable, text, vertical_space};
use iced::{Command, Element, Length, Renderer};

use iced_aw::floating_element;
use iced_aw::wrap::Wrap;
use iced_aw::Spinner;

mod searching;

#[derive(Default, PartialEq)]
enum LoadState {
    #[default]
    Loading,
    Loaded,
}

#[derive(Default)]
struct LoadStatus {
    global_series: LoadState,
    local_series: LoadState,
    shows_update: LoadState,
}

#[derive(Clone, Debug)]
pub enum Message {
    Reload,
    GlobalSeriesLoaded(Vec<SeriesMainInformation>),
    LocalSeriesLoaded(Vec<SeriesMainInformation>),
    SeriesUpdatesLoaded(Vec<SeriesMainInformation>),
    GlobalSeries(SeriesPosterMessage),
    LocalSeries(SeriesPosterMessage),
    SeriesUpdates(SeriesPosterMessage),
    Search(SearchMessage),
    SeriesSelected(Box<SeriesMainInformation>),
    ShowSearchResults,
    HideSearchResults,
    EscapeKeyPressed,
}

pub struct DiscoverTab {
    load_status: LoadStatus,
    show_search_results: bool,
    search_state: searching::Search,
    new_global_series: Vec<SeriesPoster>,
    new_local_series: Vec<SeriesPoster>,
    series_updates: Vec<SeriesPoster>,
    series_page_sender: mpsc::Sender<(series_page::Series, Command<series_page::Message>)>,
    country_name: String,
}

impl DiscoverTab {
    pub fn new(
        series_page_sender: mpsc::Sender<(series_page::Series, Command<series_page::Message>)>,
    ) -> (Self, Command<Message>) {
        (
            Self {
                load_status: LoadStatus::default(),
                show_search_results: false,
                search_state: searching::Search::default(),
                new_global_series: vec![],
                new_local_series: vec![],
                series_updates: vec![],
                series_page_sender,
                country_name: locale_settings::get_country_name_from_settings(),
            },
            load_discover_schedule_command(),
        )
    }

    pub fn refresh(&mut self) -> Command<Message> {
        let current_country_name = locale_settings::get_country_name_from_settings();
        if self.country_name != current_country_name {
            self.load_status.local_series = LoadState::Loading;
            self.country_name = current_country_name;
            load_local_aired_series()
        } else {
            Command::none()
        }
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::subscription::events_with(|event, _| {
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key_code,
                modifiers,
            }) = event
            {
                if key_code == iced::keyboard::KeyCode::Escape && modifiers.is_empty() {
                    return Some(Message::EscapeKeyPressed);
                }
                if key_code == iced::keyboard::KeyCode::F5 && modifiers.is_empty() {
                    return Some(Message::Reload);
                }
            }
            None
        })
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Reload => {
                let mut load_commands = [Command::none(), Command::none(), Command::none()];

                if let LoadState::Loaded = &self.load_status.local_series {
                    self.load_status.local_series = LoadState::Loading;
                    load_commands[0] = load_local_aired_series();
                }
                if let LoadState::Loaded = &self.load_status.global_series {
                    self.load_status.global_series = LoadState::Loading;
                    load_commands[1] = load_global_aired_series();
                }
                if let LoadState::Loaded = &self.load_status.shows_update {
                    self.load_status.shows_update = LoadState::Loading;
                    load_commands[2] = load_series_updates();
                }

                Command::batch(load_commands)
            }
            Message::GlobalSeriesLoaded(series_infos) => {
                self.load_status.global_series = LoadState::Loaded;

                let mut series_posters = Vec::with_capacity(series_infos.len());
                let mut commands = Vec::with_capacity(series_infos.len());
                for (index, series_info) in series_infos.into_iter().enumerate() {
                    let (poster, command) = SeriesPoster::new(index, series_info);
                    series_posters.push(poster);
                    commands.push(command);
                }

                self.new_global_series = series_posters;
                Command::batch(commands).map(Message::GlobalSeries)
            }
            Message::GlobalSeries(message) => {
                if let SeriesPosterMessage::SeriesPosterPressed(series_information) = message {
                    self.show_search_results = false;
                    return Command::perform(async {}, |_| {
                        Message::SeriesSelected(series_information)
                    });
                }
                self.new_global_series[message.get_index().expect("message should have an index")]
                    .update(message)
                    .map(Message::GlobalSeries)
            }
            Message::SeriesUpdatesLoaded(series) => {
                self.load_status.shows_update = LoadState::Loaded;
                let mut series_posters = Vec::with_capacity(series.len());
                let mut series_poster_commands = Vec::with_capacity(series.len());
                for (index, series_info) in series.into_iter().enumerate() {
                    let (series_poster, series_poster_command) =
                        SeriesPoster::new(index, series_info);
                    series_posters.push(series_poster);
                    series_poster_commands.push(series_poster_command);
                }
                self.series_updates = series_posters;

                Command::batch(series_poster_commands).map(Message::SeriesUpdates)
            }
            Message::SeriesUpdates(message) => {
                if let SeriesPosterMessage::SeriesPosterPressed(series_information) = message {
                    self.show_search_results = false;
                    return Command::perform(async {}, |_| {
                        Message::SeriesSelected(series_information)
                    });
                }
                self.series_updates[message.get_index().expect("message should have an index")]
                    .update(message)
                    .map(Message::SeriesUpdates)
            }
            Message::Search(message) => {
                if let SearchMessage::SeriesResultPressed(series_info) = message {
                    self.series_page_sender
                        .send(series_page::Series::new(*series_info))
                        .expect("failed to send series page");
                    self.show_search_results = false;
                    return Command::none();
                };
                self.search_state.update(message)
            }
            Message::ShowSearchResults => {
                self.show_search_results = true;
                Command::none()
            }
            Message::HideSearchResults => {
                self.show_search_results = false;
                Command::none()
            }
            Message::SeriesSelected(series_info) => {
                self.series_page_sender
                    .send(series_page::Series::new(*series_info))
                    .expect("failed to send series page");
                Command::none()
            }
            Message::LocalSeriesLoaded(series_infos) => {
                self.load_status.local_series = LoadState::Loaded;

                let mut series_posters = Vec::with_capacity(series_infos.len());
                let mut commands = Vec::with_capacity(series_infos.len());
                for (index, series_info) in series_infos.into_iter().enumerate() {
                    let (poster, command) = SeriesPoster::new(index, series_info);
                    series_posters.push(poster);
                    commands.push(command);
                }
                self.new_local_series = series_posters;
                Command::batch(commands).map(Message::LocalSeries)
            }
            Message::LocalSeries(message) => {
                if let SeriesPosterMessage::SeriesPosterPressed(series_information) = message {
                    self.show_search_results = false;
                    return Command::perform(async {}, |_| {
                        Message::SeriesSelected(series_information)
                    });
                }
                self.new_local_series[message.get_index().expect("message should have an index")]
                    .update(message)
                    .map(Message::LocalSeries)
            }
            Message::EscapeKeyPressed => {
                self.show_search_results = false;
                Command::none()
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message, Renderer> {
        let underlay: Element<'_, Message, Renderer> = scrollable(
            column!(
                series_posters_loader(
                    "Shows Airing Today Globally",
                    &self.load_status.global_series,
                    &self.new_global_series
                )
                .map(Message::GlobalSeries),
                series_posters_loader(
                    &format!("Shows Airing Today in {}", self.country_name),
                    &self.load_status.local_series,
                    &self.new_local_series
                )
                .map(Message::LocalSeries),
                series_posters_loader(
                    "Shows Updates",
                    &self.load_status.shows_update,
                    &self.series_updates
                )
                .map(Message::SeriesUpdates),
            )
            .spacing(20),
        )
        .width(Length::Fill)
        .into();

        let content = floating_element::FloatingElement::new(
            underlay,
            self.search_state.view().1.map(Message::Search),
        )
        .anchor(floating_element::Anchor::North)
        .hide(!self.show_search_results);

        column![self.search_state.view().0.map(Message::Search), content]
            .spacing(2)
            .padding(10)
            .into()
    }
}

impl DiscoverTab {
    pub fn title() -> String {
        "Discover".to_owned()
    }

    pub fn tab_label() -> troxide_widget::tabs::TabLabel {
        troxide_widget::tabs::TabLabel::new(Self::title(), BINOCULARS_FILL)
    }
}

/// Loads the locally aired series picking up the country set from the settings
fn load_local_aired_series() -> Command<Message> {
    Command::perform(
        async {
            let country_code = locale_settings::get_country_code_from_settings();
            get_series_with_country(&country_code).await
        },
        |series| Message::LocalSeriesLoaded(series.expect("failed to load series schedule")),
    )
}

/// Loads series updates
fn load_series_updates() -> Command<Message> {
    Command::perform(get_show_updates(UpdateTimestamp::Day, Some(20)), |series| {
        Message::SeriesUpdatesLoaded(series.expect("failed to load series updates"))
    })
}

/// Loads the globally aired series
fn load_global_aired_series() -> Command<Message> {
    Command::perform(get_series_with_date(None), |series| {
        Message::GlobalSeriesLoaded(series.expect("failed to load series schedule"))
    })
}

/// Loads series updates, globally and locally aired series all at once
fn load_discover_schedule_command() -> Command<Message> {
    Command::batch([
        load_series_updates(),
        load_global_aired_series(),
        load_local_aired_series(),
    ])
}

/// wraps the given series posters and places a title above them
fn series_posters_loader<'a>(
    title: &str,
    load_state: &LoadState,
    posters: &'a [SeriesPoster],
) -> Element<'a, SeriesPosterMessage, Renderer> {
    let title = text(title).size(21);

    if let LoadState::Loading = load_state {
        let spinner = container(Spinner::new())
            .center_x()
            .center_y()
            .height(100)
            .width(Length::Fill);

        return column!(title, vertical_space(10), spinner)
            .width(Length::Fill)
            .padding(10)
            .into();
    }

    if posters.is_empty() {
        let text = container(text("No Series Found"))
            .center_x()
            .center_y()
            .height(100)
            .width(Length::Fill);
        column!(title, vertical_space(10), text)
            .width(Length::Fill)
            .padding(10)
            .into()
    } else {
        let wrapped_posters =
            Wrap::with_elements(posters.iter().map(|poster| poster.normal_view()).collect())
                .spacing(5.0)
                .line_spacing(5.0)
                .padding(5.0);

        column!(title, vertical_space(10), wrapped_posters)
            .width(Length::Fill)
            .padding(10)
            .into()
    }
}