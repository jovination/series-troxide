use iced::widget::{button, checkbox, column, container, progress_bar, row, svg, text, Column};
use iced::{Command, Element, Length, Renderer};
use iced_aw::Spinner;

use self::episode_widget::Episode;

use super::Message as SeriesMessage;
use crate::core::api::episodes_information::Episode as EpisodeInfo;
use crate::core::caching::episode_list::TotalEpisodes;
use crate::core::database::AddResult;
use crate::core::{caching, database};
use crate::gui::assets::get_static_cow_from_asset;
use crate::gui::assets::icons::{ARROW_BAR_DOWN, ARROW_BAR_UP};
use episode_widget::Message as EpisodeMessage;

#[derive(Clone, Debug)]
pub enum Message {
    CheckboxPressed,
    TrackCommandComplete(Option<AddResult>),
    Expand,
    EpisodesLoaded(Vec<EpisodeInfo>),
    EpisodeAction(usize, EpisodeMessage),
}

#[derive(Clone)]
pub struct Season {
    index: usize,
    series_id: u32,
    season_number: u32,
    total_episodes: TotalEpisodes,
    episodes: Vec<episode_widget::Episode>,
    is_expanded: bool,
}

impl Season {
    pub fn new(
        index: usize,
        series_id: u32,
        season_number: u32,
        total_episodes: TotalEpisodes,
    ) -> Self {
        Self {
            index,
            series_id,
            season_number,
            total_episodes,
            episodes: vec![],
            is_expanded: false,
        }
    }
    pub fn update(&mut self, message: Message) -> Command<SeriesMessage> {
        match message {
            Message::CheckboxPressed => {
                let series_id = self.series_id;
                let season_number = self.season_number;
                let total_episodes = self.total_episodes.get_all_episodes();
                let index = self.index;

                return Command::perform(
                    async move {
                        if let Some(mut series) = database::DB.get_series(series_id) {
                            Some(
                                series
                                    .add_episodes(season_number, 1..=total_episodes as u32)
                                    .await,
                            )
                        } else {
                            None
                        }
                    },
                    move |all_newly_added| {
                        SeriesMessage::SeasonAction(
                            index,
                            Box::new(Message::TrackCommandComplete(all_newly_added)),
                        )
                    },
                );
            }
            Message::Expand => {
                self.is_expanded = !self.is_expanded;

                // preventing reloading episodes when already loaded
                // when expanding and shrinking the season widget multiple times
                if !self.episodes.is_empty() {
                    return Command::none();
                }

                let series_id = self.series_id;
                let season_number = self.season_number;
                let series_index = self.index;
                return Command::perform(
                    async move { load_episode_infos(series_id, season_number).await },
                    move |episode_infos| {
                        SeriesMessage::SeasonAction(
                            series_index,
                            Box::new(Message::EpisodesLoaded(episode_infos)),
                        )
                    },
                );
            }
            Message::EpisodesLoaded(episode_infos) => {
                let epis: Vec<(Episode, Command<Message>)> = episode_infos
                    .into_iter()
                    .enumerate()
                    .map(|(index, info)| episode_widget::Episode::new(index, self.series_id, info))
                    .collect();

                let mut commands = Vec::with_capacity(epis.len());
                let mut episodes = Vec::with_capacity(epis.len());
                for (episode, command) in epis {
                    episodes.push(episode);
                    let index = self.index;
                    commands.push(
                        command.map(move |m| SeriesMessage::SeasonAction(index, Box::new(m))),
                    );
                }

                self.episodes = episodes;
                return Command::batch(commands);
            }
            Message::EpisodeAction(index, message) => {
                let season_index = self.index;
                return self.episodes[index]
                    .update(message)
                    .map(move |m| SeriesMessage::SeasonAction(season_index, Box::new(m)));
            }
            Message::TrackCommandComplete(add_result) => {
                if let Some(AddResult::None) = add_result {
                    if let Some(mut series) = database::DB.get_series(self.series_id) {
                        series.remove_season(self.season_number);
                        series.update()
                    }
                }
            }
        }
        Command::none()
    }

    pub fn view(&self) -> Element<'_, Message, Renderer> {
        let tracked_episodes = database::DB
            .get_series(self.series_id)
            .map(|series| {
                if let Some(season) = series.get_season(self.season_number) {
                    season.get_total_episodes()
                } else {
                    0
                }
            })
            .unwrap_or(0);

        let track_checkbox = checkbox(
            "",
            (self.total_episodes.get_all_watchable_episodes() == tracked_episodes)
                && (tracked_episodes != 0),
            |_| Message::CheckboxPressed,
        );
        let season_name = text(format!("Season {}", self.season_number));

        let season_progress = progress_bar(
            0.0..=self.total_episodes.get_all_episodes() as f32,
            tracked_episodes as f32,
        )
        .height(10)
        .width(500);

        let episodes_progress = text(format!(
            "{}/{}",
            tracked_episodes,
            self.total_episodes.get_all_episodes()
        ));

        let expand_button = if self.is_expanded {
            let svg_handle = svg::Handle::from_memory(get_static_cow_from_asset(ARROW_BAR_UP));
            let up_icon = svg(svg_handle).width(Length::Shrink);
            button(up_icon).on_press(Message::Expand)
        } else {
            let svg_handle = svg::Handle::from_memory(get_static_cow_from_asset(ARROW_BAR_DOWN));
            let down_icon = svg(svg_handle).width(Length::Shrink);
            button(down_icon).on_press(Message::Expand)
        };

        let content = row!(
            track_checkbox,
            season_name,
            season_progress,
            episodes_progress,
            expand_button,
        );

        let mut content = column!(content);
        if self.is_expanded {
            if self.episodes.is_empty() {
                content = content.push(container(Spinner::new()))
            } else {
                content = content.push(Column::with_children(
                    self.episodes
                        .iter()
                        .enumerate()
                        .map(|(index, episode)| {
                            episode
                                .view()
                                .map(move |m| Message::EpisodeAction(index, m))
                        })
                        .collect(),
                ));
            }
        }

        content.into()
    }
}

async fn load_episode_infos(series_id: u32, season_number: u32) -> Vec<EpisodeInfo> {
    let episode_list = caching::episode_list::EpisodeList::new(series_id)
        .await
        .expect(&format!(
            "failed to get episodes for season {}",
            season_number
        ));

    episode_list
        .get_episodes(season_number)
        .into_iter()
        .cloned()
        .collect()
}

mod episode_widget {
    use super::Message as SeasonMessage;
    use crate::{
        core::{api::episodes_information::Episode as EpisodeInfo, caching, database},
        gui::helpers::season_episode_str_gen,
    };
    use iced::{
        widget::{checkbox, column, horizontal_space, image, row, text, Row, Text},
        Command, Element, Length, Renderer,
    };

    #[derive(Clone, Debug)]
    pub enum Message {
        ImageLoaded(Option<Vec<u8>>),
        TrackCheckboxPressed,
        TrackCommandComplete(Option<bool>),
    }

    #[derive(Clone)]
    pub struct Episode {
        index: usize,
        episode_information: EpisodeInfo,
        series_id: u32,
        episode_image: Option<Vec<u8>>,
    }

    impl Episode {
        pub fn new(
            index: usize,
            series_id: u32,
            episode_information: EpisodeInfo,
        ) -> (Self, Command<SeasonMessage>) {
            let episode_image = episode_information.image.clone();
            let episode = Self {
                index,
                episode_information,
                series_id,
                episode_image: None,
            };

            let command = if let Some(image) = episode_image {
                Command::perform(caching::load_image(image.medium_image_url), move |image| {
                    SeasonMessage::EpisodeAction(index, Message::ImageLoaded(image))
                })
            } else {
                Command::none()
            };

            (episode, command)
        }

        pub fn update(&mut self, message: Message) -> Command<SeasonMessage> {
            match message {
                Message::ImageLoaded(image) => self.episode_image = image,
                Message::TrackCheckboxPressed => {
                    let season_number = self.episode_information.season;
                    let episode_number = self.episode_information.number.unwrap();
                    let series_id = self.series_id;
                    let episode_index = self.index;

                    return Command::perform(
                        async move {
                            if let Some(mut series) = database::DB.get_series(series_id) {
                                Some(series.add_episode(season_number, episode_number).await)
                            } else {
                                None
                            }
                        },
                        move |is_newly_added| {
                            SeasonMessage::EpisodeAction(
                                episode_index,
                                Message::TrackCommandComplete(is_newly_added),
                            )
                        },
                    );
                }
                Message::TrackCommandComplete(is_newly_added) => {
                    if let Some(is_newly_added) = is_newly_added {
                        if !is_newly_added {
                            if let Some(mut series) = database::DB.get_series(self.series_id) {
                                series.remove_episode(
                                    self.episode_information.season,
                                    self.episode_information.number.unwrap(),
                                );
                            }
                        }
                    }
                }
            }
            Command::none()
        }

        pub fn view(&self) -> Element<'_, Message, Renderer> {
            let mut content = row!();
            if let Some(image_bytes) = self.episode_image.clone() {
                let image_handle = image::Handle::from_memory(image_bytes);
                let image = image(image_handle).height(60);
                content = content.push(image);
            };
            let info = column!(
                heading_widget(self.series_id, &self.episode_information),
                airdate_widget(&self.episode_information),
                airstamp_widget(&self.episode_information),
                summary_widget(&self.episode_information)
            )
            .padding(5);
            content.push(info).padding(5).width(600).into()
        }
    }

    fn summary_widget(episode_information: &EpisodeInfo) -> Text<'static, Renderer> {
        if let Some(summary) = &episode_information.summary {
            text(summary).size(15)
        } else {
            text("")
        }
    }

    fn airdate_widget(episode_information: &EpisodeInfo) -> Text<'static, Renderer> {
        if let Some(airdate) = &episode_information.airdate {
            text(airdate).size(15)
        } else {
            text("")
        }
    }

    fn airstamp_widget(episode_information: &EpisodeInfo) -> Text<'static, Renderer> {
        if let Some(airstamp) = &episode_information.airstamp {
            text(airstamp).size(15)
        } else {
            text("").size(15)
        }
    }

    fn heading_widget(
        series_id: u32,
        episode_information: &EpisodeInfo,
    ) -> Row<'static, Message, Renderer> {
        let is_tracked = database::DB
            .get_series(series_id)
            .map(|series| {
                if let Some(season) = series.get_season(episode_information.season) {
                    season.is_episode_watched(episode_information.number.unwrap())
                } else {
                    false
                }
            })
            .unwrap_or(false);

        let tracking_checkbox = checkbox("", is_tracked, |_| Message::TrackCheckboxPressed);
        row!(
            if let Some(episode_number) = episode_information.number {
                text(season_episode_str_gen(
                    episode_information.season,
                    episode_number,
                ))
            } else {
                text("")
            },
            text(&episode_information.name).size(17),
            horizontal_space(Length::Fill),
            tracking_checkbox.size(17),
        )
        .spacing(5)
    }
}
