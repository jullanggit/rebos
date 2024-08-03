// Source files to include.
mod cli; // For argument parsing and command structuring.
mod config; // Configuration stuff.
mod convert; // Convert one type to another type.
mod generation; // The generations system.
mod hook; // Hook stuff.
mod library; // Full of functions.
mod lock; // Locking file functionality.
mod management; // Stuff related to item management.
mod obj_print; // Print objects.
mod obj_print_boilerplate; // Boilerplate code for obj print.
mod places; // Where is stuff stored?
mod proc;
mod system; // Used for getting system information. // Process management stuff for Rebos.

// Import stuff from source files and crates.
use clap::Parser;
use colored::Colorize;
use config::ConfigSide;
use fspp::*;
use library::*;
use piglog::prelude::*;
use piglog::*;
use std::io::{self, Write};

// The exit code for the program.
#[derive(PartialEq)]
enum ExitCode {
    Success,
    Fail,
}

// Use this function for testing code!
fn test_code() {}

// Cleanup when Rebos fails.
fn error_cleanup() {
    let lock_state = match lock::lock_state() {
        Ok(o) => o,
        Err(_) => return,
    };
    if lock_state == lock::LockState::OnOwned {
        let _ = lock::lock_off(); // We don't have to handle this, since this is just
                                  // cleanup and the exit code will be failure anyway.
    }
}

// We are using main() to run another function, and exit according to the exit code.
fn main() -> std::process::ExitCode {
    match app() {
        ExitCode::Success => std::process::ExitCode::SUCCESS,
        ExitCode::Fail => {
            error_cleanup();

            std::process::ExitCode::FAILURE
        }
    }
}

// The "main" function.
fn app() -> ExitCode {
    // Rebos uses unique Rebos process IDs to prevent locking itself when locking other processes.
    proc::init_proc_id();

    test_code(); // This function is for nothing but testing code whilst developing!

    match is_root_user() {
        true => {
            error!("Cannot run as root! Please run as the normal user!");
            return ExitCode::Fail;
        }

        false => {}
    };

    // Migration for legacy directory location! ($HOME/.rebos-base -> $XDG_STATE_HOME/rebos)
    if places::base_legacy().exists() {
        warning!("Detected Rebos base at legacy location, moving it to new location...");
        generic!(
            "'{}' -> '{}'",
            places::base_legacy().to_string(),
            places::base().to_string()
        );

        if places::base().exists() {
            match fs_action::delete(&places::base()) {
                Ok(_) => (),
                Err(e) => {
                    fatal!(
                        "Failed to delete directory: '{}'",
                        places::base().to_string()
                    );
                    println!("{e:#?}");

                    return ExitCode::Fail;
                }
            };
        }

        match fs_action::mv(&places::base_legacy(), &places::base()) {
            Ok(_) => (),
            Err(e) => {
                fatal!(
                    "Failed to move directory ('{}') to new location: '{}'",
                    places::base_legacy().to_string(),
                    places::base().to_string()
                );
                println!("{e:#?}");

                return ExitCode::Fail;
            }
        };

        success!("Moved Rebos base directory to new location!");
    }

    let args = cli::Cli::parse();

    match &args.command {
        cli::Commands::Setup => (),
        _ => {
            if places::base().exists() == false {
                error!("It seems that the program is not set up!");
                return ExitCode::Fail;
            }
        }
    }

    #[allow(unreachable_patterns)]
    match &args.command {
        cli::Commands::Gen { command } => {
            match lock::lock_on() {
                Ok(_) => (),
                Err(_) => return ExitCode::Fail,
            };

            match command {
                cli::GenCommands::Commit(c) => {
                    info!("Committing user generation...");

                    match generation::commit(c.msg.as_str()) {
                        Ok(_) => success!("Committed generation successfully! (\"{}\")", c.msg),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::GenCommands::List => {
                    match generation::list_print() {
                        Ok(_) => (),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::GenCommands::CleanDups => {
                    match generation::management::clean_dups(true) {
                        Ok(o) => success!("Deleted {o} generations!"),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::GenCommands::Align => {
                    match generation::management::align(true) {
                        Ok(o) => success!("Aligned {o} generations!"),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::GenCommands::TidyUp => {
                    match generation::management::tidy_up() {
                        Ok(_) => (),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::GenCommands::Info => {
                    let generation = match generation::gen(ConfigSide::User) {
                        Ok(o) => o,
                        Err(_) => return ExitCode::Fail,
                    };

                    obj_print::generation(&generation);
                }
                cli::GenCommands::Latest => {
                    info!(
                        "Latest generation number is: {}",
                        match generation::latest_number() {
                            Ok(o) => o,
                            Err(_) => return ExitCode::Fail,
                        }
                    );
                }
                cli::GenCommands::DeleteOld(h) => {
                    info!("Deleting old generations...");

                    match generation::delete_old(h.how_many, true) {
                        Ok(_) => success!("Successfully deleted {} generations!", h.how_many),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::GenCommands::Delete(g) => {
                    match generation::delete(g.generation, true) {
                        Ok(_) => (), // Handled by delete().
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::GenCommands::Diff { old, new } => {
                    if generation::gen_exists(*old) == false
                        || generation::gen_exists(*new) == false
                    {
                        fatal!("Generation not found!");

                        return ExitCode::Fail;
                    }

                    let gen_1 = generation::get_gen_from_usize(*old).unwrap();
                    let gen_2 = generation::get_gen_from_usize(*new).unwrap();

                    let commit_1 = generation::get_gen_commit_from_usize(*old).unwrap();
                    let commit_2 = generation::get_gen_commit_from_usize(*new).unwrap();

                    let history = library::history_gen(&gen_1, &gen_2);

                    println!(
                        "\n{} {} {}",
                        commit_1.bright_cyan().bold(),
                        "->".bright_black().bold(),
                        commit_2.bright_cyan().bold()
                    );

                    println!("");

                    library::print_history_gen(&history);
                }
                cli::GenCommands::Current { command } => {
                    match command {
                        cli::CurrentCommands::Build => {
                            info!("Building 'current' generation...");

                            match generation::build() {
                                Ok(_) => success!("Built generation successfully!"),
                                Err(_) => return ExitCode::Fail,
                            };
                        }
                        cli::CurrentCommands::Rollback(r) => {
                            info!("Rolling back by {} generations...", r.by);

                            match generation::rollback(r.by, true) {
                                Ok(_) => success!("Rolled back successfully!"),
                                Err(_) => return ExitCode::Fail,
                            };
                        }
                        cli::CurrentCommands::ToLatest => {
                            info!("Jumping to latest generation...");

                            match generation::latest(true) {
                                Ok(_) => success!("Jumped to latest successfully!"),
                                Err(_) => return ExitCode::Fail,
                            };
                        }
                        cli::CurrentCommands::Set(s) => {
                            info!("Jumping to generation {}...", s.to);

                            match generation::set_current(s.to, true) {
                                Ok(_) => success!("Jumped to generation {} successfully!", s.to),
                                Err(_) => return ExitCode::Fail,
                            };
                        }
                        _ => {
                            error!("Command not usable yet!");
                            return ExitCode::Fail;
                        }
                    };
                }
                _ => {
                    error!("Command not usable yet!");
                    return ExitCode::Fail;
                }
            };

            match lock::lock_off() {
                Ok(_) => (),
                Err(_) => return ExitCode::Fail,
            };
        }
        cli::Commands::Setup => {
            info!("Beginning setup...");

            match setup() {
                Ok(_) => success!("Set up the program successfully!"),
                Err(_) => return ExitCode::Fail,
            };
        }
        cli::Commands::Config { command } => {
            match command {
                cli::ConfigCommands::Init => {
                    info!("Creating user configuration...");

                    match config::init_user_config() {
                        Ok(_) => success!("Created user configuration successfully!"),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::ConfigCommands::Check => {
                    let result = match config::check_config() {
                        Ok(o) => o,
                        Err(_) => return ExitCode::Fail,
                    };

                    match result {
                        Ok(misc_info) => config::print_misc_info(&misc_info),
                        Err((e, misc_info)) => {
                            config::print_errors_and_misc_info(&e, &misc_info);

                            return ExitCode::Fail;
                        }
                    };
                }
            };
        }
        cli::Commands::ForceUnlock => {
            if lock::is_lock_on() {
                piglog::warning!(
                    "Force unlocking could harm the system if done with the wrong reason!"
                );
                piglog::warning!(
                    "You should only force unlock if you know that you ABSOLUTELY need to!"
                );
                piglog::warning!(
                    "{} {} {}",
                    "Really the ONLY time you should do this is if there is only",
                    "one Rebos process running, but the locking file was never",
                    "cleaned up, so Rebos thinks there is another Rebos process!",
                );

                if bool_question("Are you REALLY sure you want to do this?", false) {
                    piglog::warning!(
                        "Force unlocking... use {} to cancel...",
                        "CTRL + C".bright_red().bold(),
                    );

                    let countdown_from: u8 = 5;

                    print!("Countdown: ");
                    for i in 0..countdown_from {
                        print!("{} ", format!("{}", countdown_from - i).bright_red().bold());
                        io::stdout().flush().unwrap();
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                    print!("\n");

                    match lock::lock_off_force() {
                        Ok(_) => piglog::success!("Unlocked Rebos!"),
                        Err(e) => {
                            piglog::fatal!("Failed to unlock: {e}");

                            return ExitCode::Fail;
                        }
                    };
                } else {
                    piglog::info!("Aborting...");

                    return ExitCode::Fail;
                }
            } else {
                piglog::info!("Not locked... skipping...");
            }
        }
        cli::Commands::IsUnlocked => {
            match lock::is_lock_on() {
                false => return ExitCode::Success,
                true => return ExitCode::Fail,
            };
        }
        cli::Commands::Managers { command, managers } => {
            match command {
                cli::ManagerCommands::Sync => {
                    match management::sync_managers(managers) {
                        Ok(_) => (),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::ManagerCommands::Upgrade { sync } => {
                    match management::upgrade_managers(*sync, managers) {
                        Ok(_) => (),
                        Err(_) => return ExitCode::Fail,
                    };
                }
                cli::ManagerCommands::RemoveOthers => {
                    match management::remove_others(managers) {
                        Ok(_) => (),
                        Err(_) => return ExitCode::Fail,
                    };
                }
            };
        }
        cli::Commands::API { command } => {
            match command {
                cli::APICommands::Echo { log_mode, message } => {
                    piglog::log_core_print(message.to_string(), *log_mode);
                }
                cli::APICommands::EchoGeneric { message } => {
                    piglog::log_generic_print(message.to_string());
                }
                cli::APICommands::BoolQuestion { question, fallback } => {
                    match bool_question(question, fallback.bool()) {
                        true => return ExitCode::Success,
                        false => return ExitCode::Fail,
                    }
                }
            };
        }
        _ => {
            error!("Command not usable yet!");
            return ExitCode::Fail;
        }
    };

    return ExitCode::Success;
}

// Function that sets up the program.
fn setup() -> Result<(), io::Error> {
    match places::setup() {
        Ok(_) => success!("Core directories verified successfully!"),
        Err(e) => return Err(e),
    };

    return Ok(());
}

// Ask for a yes or no input.
pub fn bool_question<S: AsRef<str>>(question: S, fallback: bool) -> bool {
    let question = question.as_ref();

    let (yes, no) = match fallback {
        true => ("Y".bright_green().bold().underline(), "n".bright_red()),
        false => ("y".bright_green(), "N".bright_red().bold().underline()),
    };

    loop {
        let answer = input(format!(
            "{question} [{yes}/{no}]: ",
            question = question.bright_cyan(),
        ));

        let match_on = answer.trim().to_lowercase();

        match match_on.as_str() {
            "yes" | "y" | "yeah" | "yeh" | "true" => return true,
            "no" | "n" | "nope" | "nah" | "false" => return false,
            "" => return fallback,
            _ => {
                eprintln!("Invalid response: '{}'", match_on);
            }
        }
    }
}

// Ask for user input.
pub fn input<S: AsRef<str>>(prefix: S) -> String {
    let mut answer = String::new();

    print!("{}", prefix.as_ref());

    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut answer).unwrap();

    answer
}
