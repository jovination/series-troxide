use crate::core::api::series_information;
use crate::core::{api::series_information::SeriesMainInformation, database};
use crate::gui::troxide_widget::series_poster::{Message as SeriesPosterMessage, SeriesPoster};
use iced::widget::container;
use iced_aw::{Spinner, Wrap};

use iced::Length;
use iced::{
    widget::{column, text},
    Command, Element, Renderer,
};

#[derive(Debug, Clone)]
pub enum Message {
    SeriesInformationsReceived(Vec<SeriesMainInformation>),
    SeriesSelected(Box<SeriesMainInformation>),
    SeriesPosterAction(usize, SeriesPosterMessage),
}

#[derive(Default)]
enum LoadState {
    #[default]
    Loading,
    Loaded,
}

#[derive(Default)]
pub struct MyShows {
    load_state: LoadState,
    series: Vec<SeriesPoster>,
}

impl MyShows {
    pub fn new() -> (Self, Command<Message>) {
        let series_id = database::DB.get_series_id_collection();
        let series_information = series_information::get_series_main_info_with_ids(series_id);

        (
            Self {
                load_state: LoadState::Loading,
                series: vec![],
            },
            Command::perform(series_information, |series_infos| {
                Message::SeriesInformationsReceived(series_infos)
            }),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SeriesSelected(_) => {
                unimplemented!("My shows page should not handle selecting a series poster")
            }
            Message::SeriesPosterAction(index, message) => {
                return self.series[index]
                    .update(message)
                    .map(move |message| Message::SeriesPosterAction(index, message))
            }
            Message::SeriesInformationsReceived(series_infos) => {
                self.load_state = LoadState::Loaded;

                let mut series_posters = Vec::with_capacity(series_infos.len());
                let mut series_posters_commands = Vec::with_capacity(series_infos.len());

                for (index, series_info) in series_infos.into_iter().enumerate() {
                    let (series_poster, series_poster_command) =
                        SeriesPoster::new(index, series_info);
                    series_posters.push(series_poster);
                    series_posters_commands.push(series_poster_command);
                }
                self.series = series_posters;
                Command::batch(series_posters_commands).map(|message| {
                    Message::SeriesPosterAction(message.get_id().unwrap_or(0), message)
                })
            }
        }
    }

    pub fn view(&self) -> Element<Message, Renderer> {
        let title = text("Tracked Shows").size(30);

        match self.load_state {
            LoadState::Loading => container(Spinner::new())
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into(),
            LoadState::Loaded => column!(
                title,
                Wrap::with_elements(
                    self.series
                        .iter()
                        .enumerate()
                        .map(|(index, poster)| poster
                            .view()
                            .map(move |message| { Message::SeriesPosterAction(index, message) }))
                        .collect()
                )
                .spacing(5.0)
                .padding(5.0)
            )
            .padding(5)
            .into(),
        }
    }
}
