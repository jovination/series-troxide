pub use clap::{Parser, Subcommand};

#[derive(Parser)]
#[clap(about, version, author)]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Perform actions related to series
    Series(SeriesCli),
    
    /// Add season into a series
    AddSeason(AddSeasonCli),

    /// Add episode into a series
    AddEpisode(AddEpisodeCli),

    /// Remove season from a series
    RemoveSeason(RemoveSeasonCli),

    /// Remove episode from a series
    RemoveEpisode(RemoveEpisodeCli),

    /// Remove a whole series
    RemoveSeries(RemoveSeriesCli),
}

#[derive(Parser)]
pub struct AddSeasonCli {
    /// Series name to add the season to
    pub series: String,

    /// Season number or range to be added
    pub season: u32,
}


#[derive(Parser)]
pub struct RemoveSeasonCli {
    /// Series name to remove season from
    pub series: String,

    /// Season number or range to be removed
    pub season: u32,
}

#[derive(Parser)]
pub struct AddEpisodeCli {
    /// Series name to add the episode to
    pub series: String,

    /// Season number associated
    pub season: u32,

    /// The episode number or range to be added
    pub episode: u32,       
}

#[derive(Parser)]
pub struct RemoveEpisodeCli {
    /// Series name to remove episode from
    pub series: String,

    /// Season number associated
    pub season: u32,

    /// The episode number or range to be removed
    pub episode: u32,       
}

#[derive(Parser)]
pub struct RemoveSeriesCli {
    /// The name of the series to remove
    pub series_name: String,
}

#[derive(Parser)]
pub struct SeriesCli {
    #[clap(subcommand)]
    pub command: SeriesCommand,
}

#[derive(Subcommand)]
pub enum SeriesCommand {
    /// List all the current tracked Series
    List(ListCli),

    /// Add series to the collection
    Add(SeriesAddCli),

    /// Get the summary of the specified series
    Summary(SeriesSummaryCli),
    
    /// Get the total watch time of all series
    GetTotalWatchTime(WatchTimeCli),
}

#[derive(Parser)]
pub struct ListCli {
    #[clap(subcommand)]
    pub sort_command: series_troxide::SeriesSort,
}

#[derive(Parser)]
pub struct SeriesAddCli {
    /// The name of the series
    pub name: String,

    /// The duration of episode in minutes
    pub episode_duration: u32,
}

#[derive(Parser)]
pub struct WatchTimeCli {
    #[clap(subcommand)]
    pub watch_time_command: WatchTimeCommand,
}

#[derive(Parser)]
pub struct SeriesSummaryCli {
    /// Series' name
    pub name: String,
}

#[derive(Subcommand, Clone)]
pub enum WatchTimeCommand {
    /// Watch time in seconds
    Seconds,

    /// Watch time in minutes
    Minutes,

    /// Watch time in hours
    Hours,

    /// Watch time in days
    Days,
}


