#![allow(dead_code)]

use colored::Colorize;
use fspp::*;
use piglog::prelude::*;
use piglog::*;
use serde::Deserialize;
use std::io;

use crate::library::{self, *};
use crate::places;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct ManagerConfig {
    pub many_args: bool,
    pub arg_sep: String,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            many_args: true,
            arg_sep: String::from(" "),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Manager {
    pub add: String,
    pub remove: String,
    pub sync: Option<String>,
    pub upgrade: Option<String>,
    pub config: ManagerConfig,
    pub hook_name: String,
    pub plural_name: String,
}

impl Manager {
    fn join_args(&self, items: &[String]) -> String {
        items.join(&self.config.arg_sep)
    }

    pub fn add(&self, items: &[String]) -> Result<(), io::Error> {
        let many = self.config.many_args;

        crate::hook::run_hook_and_return_if_err!(format!("pre_{}_add", self.hook_name));

        if many {
            self.add_raw(&self.join_args(items))?;
        } else {
            for i in items {
                self.add_raw(i)?;
            }
        }

        crate::hook::run_hook_and_return_if_err!(format!("post_{}_add", self.hook_name));

        Ok(())
    }

    pub fn remove(&self, items: &[String]) -> Result<(), io::Error> {
        let many = self.config.many_args;

        crate::hook::run_hook_and_return_if_err!(format!("pre_{}_remove", self.hook_name));

        if many {
            self.remove_raw(&self.join_args(items))?;
        } else {
            for i in items {
                self.remove_raw(i)?;
            }
        }

        crate::hook::run_hook_and_return_if_err!(format!("post_{}_remove", self.hook_name));

        Ok(())
    }

    fn add_raw(&self, items: &str) -> Result<(), io::Error> {
        if items.trim() == "" {
            return Ok(());
        }

        match run_command(sed(self.add.as_str(), "#:?", items).as_str()) {
            true => info!("Successfully added {}!", self.plural_name),
            false => {
                error!("Failed to add {}!", self.plural_name);

                return Err(custom_error(
                    format!("Failed to add {}!", self.plural_name).as_str(),
                ));
            }
        };

        Ok(())
    }

    fn remove_raw(&self, items: &str) -> Result<(), io::Error> {
        if items.trim() == "" {
            return Ok(());
        }

        match run_command(sed(self.remove.as_str(), "#:?", items).as_str()) {
            true => info!("Successfully removed {}!", self.plural_name),
            false => {
                error!("Failed to remove {}!", self.plural_name);

                return Err(custom_error(
                    format!("Failed to remove {}!", self.plural_name).as_str(),
                ));
            }
        };

        Ok(())
    }

    pub fn sync(&self) -> Result<(), io::Error> {
        crate::hook::run_hook_and_return_if_err!(format!("pre_{}_sync", self.hook_name));

        if let Some(ref s) = self.sync {
            match run_command(s) {
                true => info!("Synced manager successfully! ('{}')", self.plural_name),
                false => {
                    error!("Failed to sync manager! ('{}')", self.plural_name);

                    return Err(custom_error("Failed to sync repositories!"));
                }
            };
        }

        crate::hook::run_hook_and_return_if_err!(format!("post_{}_sync", self.hook_name));

        Ok(())
    }

    pub fn upgrade(&self) -> Result<(), io::Error> {
        crate::hook::run_hook_and_return_if_err!(format!("pre_{}_upgrade", self.hook_name));

        if let Some(ref s) = self.upgrade {
            match run_command(s) {
                true => info!("Successfully upgraded {}!", self.plural_name),
                false => {
                    error!("Failed to upgrade {}!", self.plural_name);

                    return Err(custom_error(
                        format!("Failed to upgrade {}!", self.plural_name).as_str(),
                    ));
                }
            };
        }

        crate::hook::run_hook_and_return_if_err!(format!("post_{}_upgrade", self.hook_name));

        Ok(())
    }

    pub fn set_plural_name(&mut self, pn: &str) {
        self.plural_name = pn.to_string();
    }

    pub fn check_config(&self) -> Result<(), Vec<String>> {
        let mut errors: Vec<String> = Vec::new();

        let valid_hook_name = fspp::filename_safe_string(&self.hook_name);

        if self.hook_name != valid_hook_name {
            errors.push(format!(
                "Field 'hook_name' must be filename safe! (Fixed version: {})",
                valid_hook_name
            ));
        }

        if errors.len() > 0 {
            return Err(errors);
        }

        Ok(())
    }
}

pub fn load_manager_no_config_check(man: &str) -> Result<Manager, io::Error> {
    let path = places::base_user().add_str(&format!("managers/{man}.toml"));

    let man_string = match file::read(&path) {
        Ok(o) => o,
        Err(e) => {
            piglog::fatal!("Failed to read manager file! ({man})");
            piglog::note!(
                "If this error shows up, it is possible the file is missing. ({})",
                path.to_string()
            );

            return Err(e);
        }
    };

    let manager: Manager = match toml::from_str(&man_string) {
        Ok(o) => o,
        Err(e) => {
            piglog::fatal!("Failed to deserialize manager! ({man})");
            piglog::fatal!("Error: {e:#?}");

            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed to deserialize manager!",
            ));
        }
    };

    Ok(manager)
}

pub fn load_manager(man: &str) -> Result<Manager, io::Error> {
    let manager = load_manager_no_config_check(man)?;

    match manager.check_config() {
        Ok(_) => (),
        Err(e) => {
            piglog::fatal!("Manager '{man}' is not configured properly! Errors:");

            for (i, error) in e.into_iter().enumerate() {
                eprintln!(
                    "{}{} {}",
                    i.to_string().bright_red().bold(),
                    ":".bright_black().bold(),
                    error
                );
            }

            return Err(io::Error::new(
                io::ErrorKind::Other,
                "Failed manager configuration check!",
            ));
        }
    };

    Ok(manager)
}

pub fn get_managers() -> Result<Vec<String>, io::Error> {
    let path = places::base_user().add_str("managers");

    let man_list: Vec<String> = directory::list_items(&path)?
        .into_iter()
        .map(|x| x.basename().replace(".toml", ""))
        .collect();

    Ok(man_list)
}

pub fn sync_managers(managers: &Option<Vec<String>>) -> Result<(), io::Error> {
    let man_names = match *managers {
        Some(ref man_names) => man_names,
        None => &get_managers()?,
    };
    for man_name in man_names {
        info!("Syncing manager {man_name}");

        let manager = load_manager(man_name)?;
        manager.sync()?;
    }
    success!("All managers synced successfully");

    Ok(())
}

pub fn upgrade_managers(
    sync_before_upgrade: bool,
    managers: &Option<Vec<String>>,
) -> Result<(), io::Error> {
    if sync_before_upgrade {
        sync_managers(managers)?;
    }

    let man_names = match *managers {
        Some(ref man_names) => man_names,
        None => &get_managers()?,
    };

    for man_name in man_names {
        info!("Upgrading manager {man_name}");

        let manager = load_manager(man_name)?;
        manager.upgrade()?;
    }

    success!("All managers upgraded successfully");

    Ok(())
}
