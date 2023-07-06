use directories::ProjectDirs;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::{
    collections::{HashMap, HashSet},
    ops::RangeInclusive,
};
use tracing::info;

use super::caching;

const DATABASE_FOLDER_NAME: &str = "series-troxide-db";

lazy_static! {
    pub static ref DB: Database = Database::init();
}

pub struct Database {
    db: Db,
}

impl Database {
    fn init() -> Self {
        info!("opening database");
        if let Some(proj_dir) = ProjectDirs::from("", "", env!("CARGO_PKG_NAME")) {
            let mut database_path = std::path::PathBuf::from(&proj_dir.data_dir());
            database_path.push(DATABASE_FOLDER_NAME);
            let db = sled::open(database_path).unwrap();
            if !db.was_recovered() {
                info!("created a fresh database as none was found");
            }
            return Self { db };
        }
        panic!("could not get the path to database");
    }

    pub fn track_series(&self, series_id: u32, series: &Series) {
        self.db
            .insert(series_id.to_string(), bincode::serialize(series).unwrap())
            .unwrap();
    }

    pub fn untrack_series(&self, series_id: u32) {
        self.db.remove(series_id.to_string()).unwrap();
    }

    pub fn get_series(&self, series_id: u32) -> Option<Series> {
        let series_bytes = self.db.get(series_id.to_string()).unwrap()?;
        Some(bincode::deserialize(&series_bytes).unwrap())
    }

    pub fn get_series_collection(&self) -> Vec<Series> {
        self.db
            .iter()
            .values()
            .map(|series| {
                let series = series.unwrap();
                bincode::deserialize(&series).unwrap()
            })
            .collect()
    }

    pub fn get_series_id_collection(&self) -> Vec<String> {
        self.db
            .iter()
            .keys()
            .map(|series| {
                let series = series.unwrap();
                // bincode::deserialize(&series).unwrap()
                String::from_utf8_lossy(&series).into_owned()
            })
            .collect()
    }

    /// get series ids and their corrensponding series structures
    pub fn get_ids_and_series(&self) -> Vec<(String, Series)> {
        self.db
            .iter()
            .map(|tup| {
                let (series_id, series) = tup.unwrap();
                let series_id = String::from_utf8_lossy(&series_id).into_owned();
                let series = bincode::deserialize::<Series>(&series).unwrap();
                (series_id, series)
            })
            .collect()
    }

    /// Returns the total number of series being tracked
    pub fn get_total_series(&self) -> usize {
        self.db.len()
    }

    /// Get the total amount of seasons watched across all
    /// series in the database
    pub fn get_total_seasons(&self) -> usize {
        self.get_series_collection()
            .iter()
            .map(|series| series.get_total_seasons())
            .sum()
    }

    /// Get the total amount of episodes watched across all
    /// series in the database
    pub fn get_total_episodes(&self) -> usize {
        self.get_series_collection()
            .iter()
            .map(|series| series.get_total_episodes())
            .sum()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Series {
    id: u32,
    name: String,
    seasons: HashMap<u32, Season>,
}

impl Series {
    pub fn new(name: String, id: u32) -> Self {
        Self {
            id,
            name,
            seasons: HashMap::new(),
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn update(&self) {
        DB.track_series(self.id, self);
    }

    pub fn add_season(&mut self, season_number: u32) {
        self.seasons.insert(season_number, Season::new());
        self.update();
    }

    pub fn remove_season(&mut self, season_number: u32) {
        self.seasons.remove(&season_number);
    }

    /// adds an episode into the series
    ///
    /// returns a true if the episode is newly added into the series and vice versa is true
    pub async fn add_episode(&mut self, season_number: u32, episode: Episode) -> bool {
        let is_newly_added = loop {
            if let Some(season) = self.seasons.get_mut(&season_number) {
                break season.track_episode(self.id, season_number, episode).await;
            } else {
                self.add_season(season_number);
            }
        };
        self.update();
        is_newly_added
    }

    pub async fn add_episodes(
        &mut self,
        season_number: u32,
        episodes_range: RangeInclusive<u32>,
    ) -> AddResult {
        let add_result = loop {
            if let Some(season) = self.seasons.get_mut(&season_number) {
                break season
                    .track_episodes(self.id, season_number, episodes_range)
                    .await;
            } else {
                self.add_season(season_number);
            }
        };
        self.update();
        add_result
    }

    /// removes an episode from the series
    pub fn remove_episode(&mut self, season_number: u32, episode_number: Episode) {
        if let Some(season) = self.seasons.get_mut(&season_number) {
            season.untrack_episode(episode_number)
        }
        self.update()
    }

    pub fn get_season(&self, season_number: u32) -> Option<&Season> {
        self.seasons.get(&season_number)
    }

    pub fn get_season_mut(&mut self, season_number: u32) -> Option<&mut Season> {
        self.seasons.get_mut(&season_number)
    }

    /// Get the total amount of seasons tracked
    pub fn get_total_seasons(&self) -> usize {
        self.seasons.len()
    }

    /// Returns total tracked episodes of the season
    pub fn get_total_episodes(&self) -> usize {
        self.seasons
            .values()
            .map(|season| season.get_total_episodes())
            .sum()
    }

    /// Return the last watched season together with it's number
    ///
    /// This obviously skip any unwatched season in between and just returns the highest
    pub fn get_last_season(&self) -> Option<(u32, &Season)> {
        self.seasons
            .iter()
            .filter(|(_, season)| season.get_total_episodes() != 0)
            .max_by(|x, y| x.0.cmp(y.0))
            .map(|(season_number, season)| (*season_number, season))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Season {
    episodes: HashSet<Episode>,
}

impl Season {
    pub fn new() -> Self {
        Self {
            episodes: HashSet::new(),
        }
    }

    /// adds the given episode to tracking
    ///
    /// tracks only when the supplied episode is watchable preventing allowing watched episodes that
    /// are released into the future.
    /// This method returns true if the episode was newly added and vice versa is true
    pub async fn track_episode(
        &mut self,
        series_id: u32,
        season_number: u32,
        episode_number: Episode,
    ) -> bool {
        let episode_list = caching::episode_list::EpisodeList::new(series_id)
            .await
            .expect("failed to get episode list");

        if let Some(episode) = episode_list.get_episode(season_number, episode_number) {
            if caching::episode_list::EpisodeList::is_episode_watchable(episode) == Some(true) {
                return self.episodes.insert(episode_number);
            }
        }
        false
    }

    /// adds a range of episode to be tracked
    ///
    /// if all episodes in the range were newly added, true is returned. if atleast one episode was not newly
    /// added i.e. it existed already before adding, false is returned.
    pub async fn track_episodes(
        &mut self,
        series_id: u32,
        season_number: u32,
        episodes_range: RangeInclusive<u32>,
    ) -> AddResult {
        let mut already_added_items = 0;
        for episode_number in episodes_range.clone() {
            if !self
                .track_episode(series_id, season_number, episode_number)
                .await
            {
                already_added_items += 1;
            };
        }

        if already_added_items == 0 {
            return AddResult::Full;
        } else if already_added_items == episodes_range.count() {
            return AddResult::None;
        } else {
            return AddResult::Partial;
        }
    }

    pub fn untrack_episode(&mut self, episode: Episode) {
        self.episodes.remove(&episode);
    }

    pub fn is_episode_watched(&self, episode: Episode) -> bool {
        self.episodes.contains(&episode)
    }

    /// Return the last watched episode
    ///
    /// This obviously skip any unwatched episode in between and just returns the highest
    pub fn get_last_episode(&self) -> Option<Episode> {
        self.episodes.iter().max().copied()
    }

    /// Get the total amount of episodes in the season
    pub fn get_total_episodes(&self) -> usize {
        self.episodes.len()
    }
}

impl Default for Season {
    fn default() -> Self {
        Self::new()
    }
}

pub type Episode = u32;

/// Indicates if adding episodes has been fully added(when none of the episodes were present before adding) or
/// partial(when some were already present) and none when all the added apisode where already present
#[derive(Debug, Clone)]
pub enum AddResult {
    /// When adding is successfully done for all items
    Full,
    /// When adding is successfully done for some items
    Partial,
    /// When adding did not happen
    None,
}
