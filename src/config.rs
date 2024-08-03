#![allow(dead_code)]

use fspp::*;
use piglog::prelude::*;
use piglog::*;
use std::io;

use crate::config;
use crate::generation;
use crate::library::*;
use crate::places;
use crate::system;

// Constants
const DEFAULT_USER_GEN: &str =
"# --------------------- #
#    Generation File    #
# --------------------- #

# Other generation files to import. (Example: intensive_apps -> ~/.config/rebos/imports/intensive_apps.toml)
imports = [
    # \"intensive_apps\",
]

# Packages to be installed via the native package manager.
[managers.system]
items = [
    # \"git\",
]

# Packages to be installed via Flatpak.
[managers.flatpak]
items = [
    # \"com.github.tchx84.Flatseal\",
]

# Packages to be installed via Cargo.
[managers.cargo]
items = [
    # \"bacon\",
]
";

const DEFAULT_PACKAGE_MANAGER_CONFIG: &str =
"# --------------------------- #
#    Manager Configuration    #
# --------------------------- #

# Make sure to enter the exact command you use as the normal user!
# That means including 'sudo' or 'doas' or whatever if the command needs it.
# Where you would put items, enter '#:?'.

# Example: add = \"sudo apt install #:?\"

add = \"\" # Example: sudo apt install #:?
remove = \"\" # Example: sudo apt remove #:?
sync = \"\" # Example: sudo apt update
upgrade = \"\" # Example: sudo apt upgrade

plural_name = \"system packages\"

hook_name = \"system_packages\" # This is used in hooks. (Example: post_system_packages_add)

# ------------------------------- #
#    Additional configuration.    #
# ------------------------------- #

# many_args = BOOL: Can you supply many items as an argument? Example: 'sudo apt install git vim wget'

[config]
many_args = true
";

const DEFAULT_FLATPAK_MANAGER_CONFIG: &str = "# Flatpak

add = \"flatpak install #:?\"
remove = \"flatpak uninstall #:?\"
upgrade = \"flatpak upgrade\"

plural_name = \"flatpaks\"

hook_name = \"flatpaks\"

[config]
many_args = true
";

const DEFAULT_CARGO_MANAGER_CONFIG: &str = "# Cargo

add = \"cargo install #:?\"
remove = \"cargo uninstall #:?\"

plural_name = \"crates\"

hook_name = \"crates\"

[config]
many_args = true
";

// This determinds if a function should
// use the files from the user's config,
// or from the base() directory.
#[derive(PartialEq, Clone, Copy)]
pub enum ConfigSide {
    User,
    System,
}

// What to grab a config file for.
pub enum Config {
    Generation,
}

// Create the user configuration.
pub fn init_user_config() -> Result<(), io::Error> {
    let system_hostname = match system::hostname() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let directories = vec![
        places::base_user(),
        places::base_user()
            .add_str("machines")
            .add_str(&system_hostname),
        places::base_user().add_str("imports"),
        places::base_user().add_str("hooks"),
        places::base_user().add_str("managers"),
    ];

    for i in directories.iter() {
        if i.exists() == false {
            match directory::create(i) {
                Ok(_o) => info!("Created directory: {}", i.to_string()),
                Err(e) => {
                    error!("Failed to create directory: {}", i.to_string());
                    return Err(e);
                }
            };
        }
    }

    let files = vec![
        (
            DEFAULT_USER_GEN,
            config::config_for(Config::Generation, ConfigSide::User),
        ),
        (
            DEFAULT_USER_GEN,
            places::base_user()
                .add_str("machines")
                .add_str(&system_hostname)
                .add_str("gen.toml"),
        ),
        (
            DEFAULT_PACKAGE_MANAGER_CONFIG,
            places::base_user().add_str("managers/system.toml"),
        ),
        (
            DEFAULT_FLATPAK_MANAGER_CONFIG,
            places::base_user().add_str("managers/flatpak.toml"),
        ),
        (
            DEFAULT_CARGO_MANAGER_CONFIG,
            places::base_user().add_str("managers/cargo.toml"),
        ),
    ];

    for i in files.iter() {
        if i.1.exists() == false {
            match file::write(i.0, &i.1) {
                Ok(_o) => info!("Created file: {}", i.1.to_string()),
                Err(e) => {
                    error!("Failed to create file: {}", i.1.to_string());
                    return Err(e);
                }
            };
        }
    }

    Ok(())
}

// Return path for a config file.
pub fn config_for(config: Config, side: ConfigSide) -> Path {
    return match config {
        Config::Generation => match side {
            ConfigSide::User => places::base_user().add_str("gen.toml"),
            ConfigSide::System => match generation::current_gen() {
                Ok(o) => o,
                Err(_e) => {
                    error!("Failed to get config path for system generation!");
                    abort();
                    return Path::new("This should never get returned.");
                }
            },
        },
    };
}

pub trait ConfigInfoToMessage {
    fn msg(&self) -> String;
}

pub enum ConfigError {
    InvalidManager(String, Vec<String>),
    MissingMachine,
}

impl ConfigInfoToMessage for ConfigError {
    fn msg(&self) -> String {
        match *self {
            Self::InvalidManager(ref man, ref errors) => {
                let mut message = format!("Manager '{man}' is not configured properly! Errors:");

                for i in errors {
                    message.push('\n');
                    message.push_str(&format!("  {}", i));
                }

                message
            }
            Self::MissingMachine => {
                format!("Missing configuration for machine! (Machine specific gen.toml...)")
            }
        }
    }
}

pub enum ConfigWarning {
    UnusedHook(String),
}

impl ConfigInfoToMessage for ConfigWarning {
    fn msg(&self) -> String {
        match *self {
            Self::UnusedHook(ref hook) => format!(
                "Hook '{hook}' is never used. (Doesn't match any manager 'hook_name' fields.)"
            ),
        }
    }
}

pub struct ConfigCheckMiscInfo {
    pub warnings: Vec<ConfigWarning>,
}

// Validate user configuration.
pub fn check_config(
) -> Result<Result<ConfigCheckMiscInfo, (Vec<ConfigError>, ConfigCheckMiscInfo)>, io::Error> {
    let mut errors: Vec<ConfigError> = Vec::new();
    let mut warnings: Vec<ConfigWarning> = Vec::new();

    let managers = match crate::management::get_managers() {
        Ok(o) => o,
        Err(e) => {
            piglog::fatal!("Failed to get a list of managers due to IO error: {}", e);

            return Err(e);
        }
    };

    let managers_loaded = {
        let mut ml: Vec<crate::management::Manager> = Vec::new();

        for i in managers.iter() {
            ml.push(match crate::management::load_manager_no_config_check(i) {
                Ok(o) => o,
                Err(e) => {
                    piglog::fatal!("Failed to load manager '{}' due to IO error: {}", i, e);

                    return Err(e);
                }
            });
        }

        ml
    };

    let hostname = system::hostname()?;

    // Check: Manager configuration.
    for man in managers.iter() {
        match crate::management::load_manager_no_config_check(man) {
            Ok(o) => {
                match o.check_config() {
                    Ok(_) => (),
                    Err(e) => errors.push(ConfigError::InvalidManager(man.to_string(), e)),
                };
            }
            Err(e) => {
                piglog::fatal!("Failed to load manager '{}' due to IO error: {}", man, e);

                return Err(e);
            }
        };
    }

    // Check: Missing machine config.
    if places::base_user()
        .add_str(&format!("machines/{}", hostname))
        .add_str("gen.toml")
        .exists()
        == false
    {
        errors.push(ConfigError::MissingMachine);
    }

    // Check: Unused hooks.
    let stages_pre: [&str; 2] = ["pre", "post"];
    let stages_suf: [&str; 4] = ["add", "remove", "sync", "upgrade"];
    let mut used_hooks: Vec<String> = Vec::new(); // A list of hook names that would be valid/used.
    used_hooks.push(String::from("pre_build"));
    used_hooks.push(String::from("post_build"));
    for man in managers_loaded {
        for pre in stages_pre {
            for suf in stages_suf {
                let hook_name = format!("{}_{}_{}", pre, man.hook_name, suf);

                used_hooks.push(hook_name);
            }
        }
    }
    for h in directory::list_items(&places::base_user().add_str("hooks"))? {
        let hook_name = h.basename();

        if used_hooks.contains(&hook_name) == false {
            warnings.push(ConfigWarning::UnusedHook(hook_name));
        }
    }

    let warnings_len = warnings.len();

    let misc_info = ConfigCheckMiscInfo { warnings };

    if errors.len() > 0 {
        return Ok(Err((errors, misc_info)));
    }

    if warnings_len == 0 {
        piglog::success!("Configuration has no errors or warnings! (^-^)");
    }

    Ok(Ok(misc_info))
}

pub fn print_misc_info(misc_info: &ConfigCheckMiscInfo) {
    for w in misc_info.warnings.iter() {
        piglog::warning!("{}", w.msg());
    }
}

pub fn print_errors_and_misc_info(errors: &[ConfigError], misc_info: &ConfigCheckMiscInfo) {
    print_misc_info(misc_info);

    for e in errors {
        piglog::error!("{}", e.msg());
    }
}
