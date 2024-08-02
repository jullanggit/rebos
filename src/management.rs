#![allow(dead_code)]

use colored::Colorize;
use fspp::*;
use piglog::prelude::*;
use piglog::*;
use serde::Deserialize;
use std::io;

use crate::config::ConfigSide;
use crate::generation::gen;
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
    pub list: Option<String>,
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

    pub fn remove_other(&self, items: &[String]) -> Result<(), io::Error> {
        if self.list.is_some() {
            let mut others = self.list()?;
            others.retain(|other| !items.contains(other));
            dbg!(&others);

            self.remove(&others)
        } else {
            Ok(())
        }
    }

    /// Gets a list of installed {plural_name}
    /// Expects that the list command exists for the manager
    pub fn list(&self) -> Result<Vec<String>, io::Error> {
        let list_cmd = self.list.as_ref().expect("Command should exist");

        match run_command_with_output(list_cmd) {
            Some(output) => Ok(output.split_whitespace().map(|s| s.to_owned()).collect()),
            None => {
                let error = format!("Failed to get list of {}!", self.plural_name);

                error!("{error}");

                Err(custom_error(&error))
            }
        }
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

pub fn managers() -> Result<Vec<String>, io::Error> {
    let path = places::base_user().add_str("managers");

    let man_list: Vec<String> = directory::list_items(&path)?
        .into_iter()
        .map(|x| x.basename().replace(".toml", ""))
        .collect();

    Ok(man_list)
}

pub fn sync_all() -> Result<(), io::Error> {
    let m_all = managers()?;
    let m_len = m_all.len();

    info!("Syncing {} managers...", m_len);

    for m in m_all {
        let man = load_manager(&m)?;

        man.sync()?;
    }

    success!("All {} managers synced successfully!", m_len);

    Ok(())
}

pub fn upgrade_all(sync_before_upgrade: bool) -> Result<(), io::Error> {
    if sync_before_upgrade {
        sync_all()?;
    }

    let m_all = managers()?;
    let m_len = m_all.len();

    info!("Upgrading {} managers...", m_len);

    for m in m_all {
        let man = load_manager(&m)?;

        man.upgrade()?;
    }

    success!("All {} managers upgraded successfully!", m_len);

    Ok(())
}

// TODO: add info and success messages
pub fn remove_other_all() -> Result<(), io::Error> {
    let curr_gen = gen(ConfigSide::System)?;

    for (m, items) in curr_gen.managers.iter() {
        let man = load_manager(m)?;

        man.remove_other(&items.items)?;
    }

    Ok(())
}
