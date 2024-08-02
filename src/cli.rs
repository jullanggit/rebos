#![allow(dead_code)]

use clap::{Parser, Subcommand, ValueEnum};
use piglog::LogMode;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
/// Make any Linux distribution repeatable!
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Run a generation system command
    Gen {
        #[command(subcommand)]
        command: GenCommands,
    },
    /// Run the program setup
    Setup,
    /// Configuration commands
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Force Rebos to unlock (this could break your system if done without reason)
    ForceUnlock,
    /// Is Rebos unlocked? (Exit Status: (0 = Yes, 1 = No))
    IsUnlocked,
    /// Manager commands
    Managers {
        #[command(subcommand)]
        command: ManagerCommands,
    },
    /// API for things like scripting
    API {
        #[command(subcommand)]
        command: APICommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum APICommands {
    /// Use the Rebos log message system
    Echo { log_mode: LogMode, message: String },
    /// Use the Rebos log message system (Generic)
    EchoGeneric { message: String },
    /// Use Rebos to ask the user for a boolean yes or no question (Exit Status: (0 = Yes, 1 = No))
    BoolQuestion {
        /// Question to be asked
        question: String,
        /// Fallback for when the user simply presses enter to accept the default
        fallback: CLIBoolean,
    },
}

#[derive(Subcommand, Debug)]
pub enum ManagerCommands {
    /// Sync all managers
    Sync,
    /// Upgrade all managers
    Upgrade {
        #[clap(long)]
        /// Sync before upgrading
        sync: bool,
    },
    /// Remove everything except for what is specified in the config for all managers
    RemoveOther,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Create a default Rebos configuration
    Init,
    /// Check for warnings and errors in the Rebos configuration
    Check,
}

#[derive(Subcommand, Debug)]
pub enum GenCommands {
    /// Confirm your custom generation, and make it the 'current' generation
    Commit(Commit),
    /// List all system generations
    List,
    /// Delete duplicate generations
    CleanDups,
    /// Align all generation numbers (Example: [1, 4, 22, 35] -> [1, 2, 3, 4])
    Align,
    /// Automatically clean up the generations (clean-dups, align, etc...)
    TidyUp,
    /// Get information on the generation in the user's config
    Info,
    /// Print out what the latest system generation number is
    Latest,
    /// Delete older generations
    DeleteOld(GenDeleteOld),
    /// Delete a specific generation
    Delete(GenDelete),
    /// The difference between 2 generations
    Diff {
        /// Generation to act as base
        old: usize,
        /// Generation to act as changes
        new: usize,
    },
    /// Command related to the 'current' generation
    Current {
        #[command(subcommand)]
        command: CurrentCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum CurrentCommands {
    /// Build the 'current' generation (You can always roll back later)
    Build,
    /// Rollback to a previous generation (You still need to build after rolling back)
    Rollback(Rollback),
    /// Set the 'current' generation to the latest generation
    ToLatest,
    /// Set the 'current' generation to a specific generation
    Set(SetCurrent),
}

#[derive(ValueEnum, Debug, Clone, Copy)]
// The only reason this enum exists is because Clap bugs out when asked for a `bool`.
pub enum CLIBoolean {
    Yes,
    No,
}

impl CLIBoolean {
    #[inline(always)]
    pub fn bool(&self) -> bool {
        match self {
            Self::Yes => true,
            Self::No => false,
        }
    }
}

#[derive(Parser, Debug)]
pub struct GenDelete {
    /// The generation to delete
    pub generation: usize,
}

#[derive(Parser, Debug)]
pub struct GenDeleteOld {
    /// Starting at the oldest generation, how many should be deleted?
    pub how_many: usize,
}

#[derive(Parser, Debug)]
pub struct Commit {
    /// The commit message shows up in the list command
    pub msg: String,
}

#[derive(Parser, Debug)]
pub struct SetCurrent {
    /// Generation to jump to
    pub to: usize,
}

#[derive(Parser, Debug)]
pub struct Rollback {
    /// How many generations to rollback by
    pub by: isize,
}
