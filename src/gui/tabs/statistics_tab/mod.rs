use std::sync::mpsc;

use iced::widget::{column, container, row, scrollable};
use iced::{Command, Element, Length, Renderer};
use iced_aw::Wrap;

use crate::core::{api::series_information::SeriesMainInformation, database};
use crate::gui::assets::icons::GRAPH_UP_ARROW;
use series_banner::{
    IndexedMessage as SeriesBannerIndexedMessage, Message as SeriesBannerMessage, SeriesBanner,
};

use mini_widgets::*;

mod mini_widgets;

#[derive(Clone, Debug)]
pub enum Message {
    SeriesInfosAndTimeReceived(Vec<(SeriesMainInformation, Option<u32>)>),
    SeriesBanner(SeriesBannerIndexedMessage<SeriesBannerMessage>),
}

pub struct StatisticsTab {
    series_infos_and_time: Vec<(SeriesMainInformation, Option<u32>)>,
    series_banners: Vec<SeriesBanner>,
    series_page_sender: mpsc::Sender<SeriesMainInformation>,
}

impl StatisticsTab {
    pub fn new(
        series_page_sender: mpsc::Sender<SeriesMainInformation>,
    ) -> (Self, Command<Message>) {
        (
            Self {
                series_infos_and_time: vec![],
                series_banners: vec![],
                series_page_sender,
            },
            Command::perform(
                get_series_with_runtime(),
                Message::SeriesInfosAndTimeReceived,
            ),
        )
    }

    pub fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::SeriesInfosAndTimeReceived(mut series_infos_and_time) => {
                self.series_infos_and_time = series_infos_and_time.clone();

                series_infos_and_time.sort_by(|(_, average_minutes_a), (_, average_minutes_b)| {
                    average_minutes_b.cmp(average_minutes_a)
                });

                let mut banners = Vec::with_capacity(series_infos_and_time.len());
                let mut banners_commands = Vec::with_capacity(series_infos_and_time.len());
                for (index, series_info_and_time) in series_infos_and_time.into_iter().enumerate() {
                    let (banner, banner_command) = SeriesBanner::new(
                        index,
                        series_info_and_time,
                        self.series_page_sender.clone(),
                    );
                    banners.push(banner);
                    banners_commands.push(banner_command);
                }
                self.series_banners = banners;
                Command::batch(banners_commands).map(Message::SeriesBanner)
            }
            Message::SeriesBanner(message) => {
                self.series_banners[message.index()].update(message);
                Command::none()
            }
        }
    }
    pub fn view(&self) -> Element<Message, Renderer> {
        let series_list = Wrap::with_elements(
            self.series_banners
                .iter()
                .map(|banner| banner.view().map(Message::SeriesBanner))
                .collect(),
        )
        .spacing(5.0)
        .line_spacing(5.0);

        let series_list = container(series_list).width(Length::Fill).center_x();

        let content = column![
            row![watch_count(), time_count(&self.series_infos_and_time)].spacing(10),
            series_list
        ]
        .spacing(10)
        .padding(10);

        container(scrollable(content))
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

/// Get the collection of all series with their associated total
/// average runtime
async fn get_series_with_runtime() -> Vec<(SeriesMainInformation, Option<u32>)> {
    let series_ids_handles: Vec<_> = database::DB
        .get_series_collection()
        .into_iter()
        .map(|series| tokio::spawn(async move { series.get_total_average_watchtime().await }))
        .collect();

    let mut infos_and_time = Vec::with_capacity(series_ids_handles.len());
    for handle in series_ids_handles {
        infos_and_time.push(
            handle
                .await
                .expect("failed to await all series_infos and their average runtime"),
        );
    }
    infos_and_time
}

impl StatisticsTab {
    pub fn title() -> String {
        "Statistics".to_owned()
    }

    pub fn tab_label() -> super::TabLabel {
        super::TabLabel::new(Self::title(), GRAPH_UP_ARROW)
    }
}
