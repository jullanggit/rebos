#![allow(dead_code)]

pub mod management;

use std::io;
use colored::Colorize;
use serde::{Serialize, Deserialize};
use hashbrown::HashMap;
use piglog::prelude::*;
use piglog::*;
use fspp::*;

use crate::hook::run_hook_and_return_if_err;
use crate::places;
use crate::library;
use crate::library::*;
use crate::package_management::PackageManager;
use crate::config::{Config, ConfigSide};
use crate::config::config_for;
use crate::system;

#[derive(PartialEq, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Packages {
    pub pkgs: Vec<String>,
}

impl Default for Packages {
    fn default() -> Self {
        Self {
            pkgs: Vec::new(),
        }
    }
}

#[derive(PartialEq, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct GenerationLegacy {
    pub imports: Vec<String>,
    pub pkgs: Vec<String>,
    pub flatpaks: Vec<String>,
    pub crates: Vec<String>,
}

impl GenerationLegacy {
    pub fn migrate(self) -> Generation {
        let mut gen = Generation::default();

        gen.imports = self.imports;

        gen.pkg_managers.insert("system".to_string(), Packages { pkgs: self.pkgs });
        gen.pkg_managers.insert("flatpak".to_string(), Packages { pkgs: self.flatpaks });
        gen.pkg_managers.insert("cargo".to_string(), Packages { pkgs: self.crates });

        gen
    }
}

impl Default for GenerationLegacy {
    fn default() -> Self {
        Self {
            imports: Vec::new(),
            pkgs: Vec::new(),
            flatpaks: Vec::new(),
            crates: Vec::new(),
        }
    }
}

// The structure for a generation.
#[derive(PartialEq, Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields, default)]
pub struct Generation {
    pub imports: Vec<String>,
    pub pkg_managers: HashMap<String, Packages>,
}

impl Default for Generation {
    fn default() -> Generation {
        Generation {
            imports: Vec::new(),
            pkg_managers: HashMap::new(),
        }
    }
}

impl GenerationUtils for Generation {
    fn extend(&mut self, other_gen: Generation) {
        self.imports.extend(other_gen.imports);

        for i in other_gen.pkg_managers.keys() {
            match self.pkg_managers.get_mut(i) {
                Some(s) => s.pkgs.extend(other_gen.pkg_managers.get(i).unwrap().pkgs.clone()),
                None => {
                    self.pkg_managers.insert(i.to_string(), Packages { pkgs: Vec::new() });
                    self.pkg_managers.get_mut(i).unwrap().pkgs.extend(other_gen.pkg_managers.get(i).unwrap().pkgs.clone());
                },
            };
        }
    }
}

pub trait GenerationUtils {
    // Extend all of the fields from one Generation object to another, another being the caller.
    fn extend(&mut self, other_gen: Generation);
}

// Return generation structure for...
pub fn gen(side: ConfigSide) -> Result<Generation, io::Error> {
    let mut generation = match read_to_gen(&config_for(Config::Generation, side)) {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let system_hostname = match system::hostname() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    if side == ConfigSide::User {
        generation.extend(read_to_gen(&places::base_user().add_str("machines").add_str(&system_hostname).add_str("gen.toml"))?);
    }

    while generation.imports.len() > 0 {
        let gen_imports = generation.imports.clone();

        for i in gen_imports.iter() {
            let i_gen = read_to_gen(&places::base_user().add_str("imports").add_str(&format!("{i}.toml")))?;

            generation.extend(i_gen);
        }

        let after_gen_imports = generation.imports.clone();

        for i in 0..after_gen_imports.len() {
            if gen_imports.contains(&after_gen_imports[i]) {
                generation.imports[i] = String::new();
            }
        }

        generation.imports = generation.imports.into_iter().filter(|x| *x != String::new()).collect();
    }

    Ok(generation)
}

// Read a file and return a Generation object.
fn read_to_gen(path: &Path) -> Result<Generation, io::Error> {
    let gen_string = match file::read(path) {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to read generation TOML file!");
            return Err(e);
        },
    };

    Ok(match toml::from_str(&gen_string) {
        Ok(o) => o,
        Err(e) => {
            warning!("Failed to deserialize generation, attempting legacy mode... ('{}')", path.to_string());

            let legacy: GenerationLegacy = match toml::from_str(&gen_string) {
                Ok(o) => o,
                Err(_) => {
                    error!("Failed to deserialize in legacy mode, here is the error for regular mode:");
                    error!("{e:#?}");

                    return Err(custom_error("Failed to deserialize generation!"));
                },
            };

            legacy.migrate()
        },
    })
}

// Does the generation specified exist?
pub fn gen_exists(gen_id: usize) -> bool {
    let path = places::gens().add_str(&gen_id.to_string()).add_str("gen.toml");

    path.exists()
}

// Get generation for the id
pub fn get_gen_from_usize(gen_id: usize) -> Result<Generation, io::Error> {
    let generation = read_to_gen(&places::gens().add_str(&gen_id.to_string()).add_str("gen.toml"))?;

    return Ok(generation);
}

// Get generation commit for the id
pub fn get_gen_commit_from_usize(gen_id: usize) -> Result<String, io::Error> {
    let gen_commit = file::read(&places::gens().add_str(&gen_id.to_string()).add_str("commit"))?;

    return Ok(gen_commit);
}

// Get latest generation number.
pub fn latest_number() -> Result<usize, io::Error> {
    let generation_numbers = match list_gen_nums() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    if generation_numbers.len() < 1 {
        return Ok(0);
    }

    let latest_num = match generation_numbers.into_iter().max() {
        Some(s) => s,
        None => {
            error!("Failed to get max number in generation numbers list!");
            return Err(custom_error("Failed to get max number in generation number list!"));
        },
    };

    return Ok(latest_num);
}

// Create a new system generation based on the user generation.
pub fn commit(msg: &str) -> Result<(), io::Error> {
    let generation_number = match latest_number() {
        Ok(o) => o,
        Err(e) => return Err(e),
    } + 1;

    let gen_dir = places::gens().add_str(&generation_number.to_string());

    match directory::create(&gen_dir) {
        Ok(_o) => info!("Created generation directory."),
        Err(e) => {
            error!("Failed to create generation directory!");
            return Err(e);
        },
    };

    let user_gen = match gen(ConfigSide::User) {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let user_gen_string = match toml::to_string(&user_gen) {
        Ok(o) => o,
        Err(_e) => {
            error!("Failed to convert user generation to string!");
            return Err(custom_error("Failed to convert user generation to string!"));
        },
    };

    let files = vec![
        (msg, gen_dir.add_str("commit")),
        (user_gen_string.as_str(), gen_dir.add_str("gen.toml")),
    ];

    for i in files.iter() {
        match file::write(i.0, &i.1) {
            Ok(_o) => info!("Created file: {}", i.1.basename()),
            Err(e) => {
                error!("Failed to create file: {}", i.1.basename());
                return Err(e);
            },
        };
    }

    match set_current(generation_number, true) {
        Ok(_o) => {},
        Err(e) => return Err(e),
    };

    return Ok(());
}

fn load_package_manager(man: &str) -> Result<PackageManager, io::Error> {
    let path = places::base_user().add_str(&format!("pkg_managers/{man}.toml"));

    let man_string = match file::read(&path) {
        Ok(o) => o,
        Err(e) => {
            piglog::fatal!("Failed to read package manager file! ({man})");
            piglog::note!("If this error shows up, it is possible the file is missing. ({})", path.to_string());

            return Err(e);
        },
    };

    let package_manager: PackageManager = match toml::from_str(&man_string) {
        Ok(o) => o,
        Err(e) => {
            piglog::fatal!("Failed to deserialize package manager! ({man})");
            piglog::fatal!("Error: {e:#?}");

            return Err(io::Error::new(io::ErrorKind::Other, "Failed to deserialize package manager!"));
        },
    };

    Ok(package_manager)
}

// Build the 'current' system generation.
pub fn build() -> Result<(), io::Error> {
    run_hook_and_return_if_err!("pre_build");

    let current_num = match get_current() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let curr_gen = match gen(ConfigSide::System) {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    match file::read(&places::gens().add_str("built")) {
        Ok(o) => {
            let built_gen = read_to_gen(&places::gens().add_str(o.trim()).add_str("gen.toml"))?;

            let mut summary_entries: HashMap<String, Vec<History>> = HashMap::new();

            // Add new packages, remove old packages.
            for i in curr_gen.pkg_managers.keys() {
                let pkg_man = load_package_manager(i)?;

                let curr_pkgs = curr_gen.pkg_managers.get(i).unwrap();

                match built_gen.pkg_managers.get(i) {
                    Some(built_pkgs) => {
                        let diffs = history(&built_pkgs.pkgs, &curr_pkgs.pkgs);

                        let mut to_install: Vec<String> = Vec::new();
                        let mut to_remove: Vec<String> = Vec::new();

                        for j in diffs.iter() {
                            match j.mode {
                                HistoryMode::Add => to_install.push(j.line.to_string()),
                                HistoryMode::Remove => to_remove.push(j.line.to_string()),
                            };
                        }

                        pkg_man.install(&to_install)?;
                        pkg_man.remove(&to_remove)?;

                        summary_entries.insert(i.to_string(), diffs);
                    },
                    None => {
                        pkg_man.install(&curr_pkgs.pkgs)?;

                        summary_entries.insert(i.to_string(), curr_pkgs.pkgs.iter().map(|x| History {
                            mode: HistoryMode::Add,
                            line: x.to_string(),
                        }).collect());
                    },
                }
            }

            // Remove packages from package managers that were removed from the generation.
            for i in built_gen.pkg_managers.keys() {
                let built_pkgs = built_gen.pkg_managers.get(i).unwrap();

                match curr_gen.pkg_managers.get(i) {
                    Some(_) => (),
                    None => {
                        let pkg_man = load_package_manager(i)?;

                        pkg_man.remove(&built_pkgs.pkgs)?;

                        summary_entries.insert(i.to_string(), built_pkgs.pkgs.iter().map(|x| History {
                            mode: HistoryMode::Remove,
                            line: x.to_string(),
                        }).collect());
                    },
                };
            }

            println!("");
            println!("");
            println!("");

            info!("#################");
            info!("#    SUMMARY    #");
            info!("#################");

            println!("");

            library::print_history_gen(&summary_entries);

            println!("");
            println!("");
        },
        Err(_) => {
            for i in curr_gen.pkg_managers.keys() {
                let curr_pkgs = curr_gen.pkg_managers.get(i).unwrap();

                let pkg_man = load_package_manager(i)?;

                pkg_man.install(&curr_pkgs.pkgs)?;
            }

            note!("Since this is your first time building a generation, there is no summary.");
        },
    };

    match set_built(current_num, true) {
        Ok(_o) => {},
        Err(e) => return Err(e),
    };

    run_hook_and_return_if_err!("post_build");

    Ok(())
}

// Set the 'current' generation to another older generation.
pub fn rollback(by: isize, verbose: bool) -> Result<(), io::Error> {
    let current_num = match get_current() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let new_current = (current_num as isize) - by;

    match set_current(new_current as usize, verbose) {
        Ok(_o) => {},
        Err(e) => return Err(e),
    };

    return Ok(());
}

// Set the 'current' generation to the latest generation.
pub fn latest(verbose: bool) -> Result<(), io::Error> {
    match set_current(match latest_number() {
        Ok(o) => o,
        Err(e) => return Err(e),
    }, verbose) {
        Ok(_o) => {},
        Err(e) => return Err(e),
    };

    return Ok(());
}

// Set the 'current' generation to a specific generation.
pub fn set_current(to: usize, verbose: bool) -> Result<(), io::Error> {
    if to > match latest_number() {
        Ok(o) => o,
        Err(e) => return Err(e),
    } {
        error!("Out of range! Too high!");
        return Err(custom_error("Out of range!"));
    }

    if to < 1 {
        error!("Out of range! Too low!");
        return Err(custom_error("Out of range!"));
    }

    match file::write(to.to_string().trim(), &places::gens().add_str("current")) {
        Ok(_) => {
            if verbose {
                info!("Set 'current' to: {}", to);
            }

            return Ok(());
        },
        Err(e) => {
            error!("Failed to create/write 'current' tracking file!");
            return Err(e);
        },
    };
}

// Set the 'built' generation to a specific generation.
pub fn set_built(to: usize, verbose: bool) -> Result<(), io::Error> {
    if to > match latest_number() {
        Ok(o) => o,
        Err(e) => return Err(e),
    } {
        error!("Out of range! Too high!");
        return Err(custom_error("Out of range!"));
    }

    if to < 1 {
        error!("Out of range! Too low!");
        return Err(custom_error("Out of range!"));
    }

    match file::write(to.to_string().trim(), &places::gens().add_str("built")) {
        Ok(_o) => {
            if verbose {
                info!("Set 'built' to: {}", to);
            }

            return Ok(());
        },
        Err(e) => {
            error!("Failed to create/write 'built' tracking file!");
            return Err(e);
        },
    };
}

// Get the 'current' generation number.
pub fn get_current() -> Result<usize, io::Error> {
    let contents = match file::read(&places::gens().add_str("current")) {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to read 'current' file!");
            return Err(e);
        },
    };

    let generation: usize = match contents.trim().parse() {
        Ok(o) => o,
        Err(_e) => {
            error!("Failed to parse number from 'current' file! (Maybe 'current' file is corrupted?)");
            return Err(custom_error("Failed to parse number out of 'current' file!"));
        },
    };

    return Ok(generation);
}

// Get the currently built generation number. (With output.)
pub fn get_built() -> Result<usize, io::Error> {
    return get_built_core(true);
}

// Get the currently built generation number. (Without output.)
pub fn get_built_no_output() -> Result<usize, io::Error> {
    return get_built_core(false);
}

// Get the currently built generation number. (CORE)
pub fn get_built_core(output: bool) -> Result<usize, io::Error> {
    let contents = match file::read(&places::gens().add_str("built")) {
        Ok(o) => o,
        Err(e) => {
            if output {
                error!("Failed to read 'built' file!");
            }

            return Err(e);
        },
    };

    let generation: usize = match contents.trim().parse() {
        Ok(o) => o,
        Err(_e) => {
            if output {
                error!("Failed to parse number from 'built' file! (Maybe 'built' file is corrupted?)");
            }

            return Err(custom_error("Failed to parse number out of 'built' file!"));
        },
    };

    return Ok(generation);
}

// Has a generation been built yet?
pub fn been_built() -> bool {
    return places::gens().add_str("built").exists();
}

// Delete old generations.
pub fn delete_old(how_many: usize, verbose: bool) -> Result<(), io::Error> {
    let offset = match get_oldest() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let latest_gen = match latest_number() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    for i in offset..(how_many + offset) {
        if i > latest_gen {
            break;
        }

        match delete(i, verbose) {
            Ok(_) => (), // This is a rare instance where the matched function actually did the info!() itself!
            Err(e) => return Err(e),
        };
    }

    return Ok(());
}

// Delete a specific generation.
pub fn delete(generation: usize, verbose: bool) -> Result<(), io::Error> {
    if match is_current(generation) {
        Ok(o) => o,
        Err(e) => return Err(e),
    } {
        warning!("Could not delete generation {}, because it is the 'current' generation, and is protected!", generation);
        return Ok(());
    }

    if been_built() {
        if match is_built(generation) {
            Ok(o) => o,
            Err(e) => return Err(e),
        } {
            warning!("Could not delete generation {}, because it is the currently built generation, and is protected!", generation);
            return Ok(());
        }
    }

    if match exists(generation) {
        Ok(o) => o,
        Err(e) => return Err(e),
    } == false {
        error!("Generation {} does not exist!", generation);
        return Err(custom_error("Generation does not exist!"));
    }

    match fs_action::delete(&places::gens().add_str(&generation.to_string())) {
        Ok(_) => {
            if verbose {
                info!("Deleted generation: {}", generation);
            }
        },
        Err(e) => {
            error!("Failed to delete generation: {}", generation);
            return Err(e);
        },
    };

    return Ok(());
}

// Move a generation to another spot. (Number -> Number)
pub fn move_gen(from: usize, to: usize, verbose: bool) -> Result<(), io::Error> {
    let current = is_current(from)?;
    let built = is_built(from)?;

    let from_path = places::gens().add_str(&from.to_string());
    let to_path = places::gens().add_str(&to.to_string());

    fs_action::mv(&from_path, &to_path)?;

    if verbose {
        info!("Moved generation: {from} -> {to}");
    }

    if current {
        set_current(to, verbose)?;
    }

    if built {
        set_built(to, verbose)?;
    }

    return Ok(());
}

// See if a generation exists.
pub fn exists(generation: usize) -> Result<bool, io::Error> {
    let gen_nums = match list_gen_nums() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    return Ok(gen_nums.contains(&generation));
}

// List generation numbers.
pub fn list_gen_nums() -> Result<Vec<usize>, io::Error> {
    let gen_list = match list_with_no_calls() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let mut gen_nums: Vec<usize> = Vec::new();

    for i in gen_list.iter() {
        gen_nums.push(match usize_from_gen_name(i.0.as_str()) {
            Ok(o) => o,
            Err(e) => return Err(e),
        });
    }

    return Ok(gen_nums);
}

// Parse generation name to usize.
pub fn usize_from_gen_name(name: &str) -> Result<usize, io::Error> {
    return Ok(match name.trim().parse() {
        Ok(o) => o,
        Err(_e) => {
            error!("Failed to parse invalid generation name! ({})", name.trim());
            return Err(custom_error("Failed to parse invalid generation name!"));
        },
    });
}

// Return true or false based on if the given generation is the 'current' generation.
pub fn is_current(generation: usize) -> Result<bool, io::Error> {
    if generation == match get_current() {
        Ok(o) => o,
        Err(e) => return Err(e),
    } {
        return Ok(true);
    }

    else {
        return Ok(false);
    }
}

// Return true or false based on if the given generation is the built generation.
pub fn is_built(generation: usize) -> Result<bool, io::Error> {
    if generation == match get_built() {
        Ok(o) => o,
        Err(e) => return Err(e),
    } {
        return Ok(true);
    }

    else {
        return Ok(false);
    }
}

// List all generations. (NORMAL)
pub fn list() -> Result<Vec<(String, String, bool, bool)>, io::Error> {
    return list_core(true);
}

// List all generations. (ISOLATED MODE | For avoiding errors with called un-needed functions!)
pub fn list_with_no_calls() -> Result<Vec<(String, String, bool, bool)>, io::Error> {
    return list_core(false);
}

// List all generations. (CORE)
fn list_core(calls: bool) -> Result<Vec<(String, String, bool, bool)>, io::Error> {
    let gen_listed = match directory::list_items(&places::gens()) {
        Ok(o) => o,
        Err(e) => {
            error!("Failed to list the generations directory! ({})", places::gens().to_string());
            return Err(e);
        },
    };

    let mut generations: Vec<Path> = Vec::new();

    for i in gen_listed.into_iter() {
        match i.path_type() {
            PathType::File => {},
            PathType::Directory => generations.push(i),
            PathType::Invalid => {
                error!("Found invalid path! (Listing generations.)");
                return Err(custom_error("Found invalid path."));
            },
        };
    }

    let mut gens_with_info: Vec<(String, String, bool, bool)> = Vec::new();

    for i in generations.iter() {
        let generation_name = i.basename();
        let commit_msg = file::read(&i.add_str("commit")).unwrap_or(String::from("<< COMMIT MESSAGE MISSING >>"));

        let current_number: usize;
        let built_number: usize;

        if calls == true {
            current_number = match get_current() {
                Ok(o) => o,
                Err(e) => return Err(e),
            };
            built_number = match get_built_no_output() {
                Ok(o) => o,
                Err(_e) => 0,
            };
        } else {
            current_number = 0;
            built_number = 0;
        }

        let is_current: bool;
        let is_built: bool;

        if generation_name == current_number.to_string() {
            is_current = true;
        } else {
            is_current = false;
        }

        if generation_name == built_number.to_string() {
            is_built = true;
        } else {
            is_built = false;
        }

        gens_with_info.push((generation_name, commit_msg, is_current, is_built));
    }

    return Ok(gens_with_info);
}

// Print out the list of generations.
pub fn list_print() -> Result<(), io::Error> {
    let list_items = match list() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let list_items_sorted = match sort_list_vector(&list_items) {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    let mut max_digits: usize = 0;

    if list_items_sorted.len() > 0 {
        max_digits = list_items_sorted[list_items_sorted.len() - 1].0.to_string().trim().len();
    }

    for i in list_items_sorted.iter() {
        let mut misc_text = String::new();

        if i.2 {
            misc_text.push_str(format!(" {}{}{}", "[".bright_black().bold(), "CURRENT".bright_green().bold(), "]".bright_black().bold()).as_str());
        }

        if i.3 {
            misc_text.push_str(format!(" {}{}{}", "[".bright_black().bold(), "BUILT".bright_yellow().bold(), "]".bright_black().bold()).as_str());
        }

        let mut tabbed = String::new();

        for _j in 0..(max_digits - i.0.trim().len()) {
            tabbed.push_str(" ");
        }

        generic!("{}{} ... ({}){}", tabbed, i.0, i.1, misc_text);
    }

    return Ok(());
}

// Get only list vector generation names.
fn get_list_vector_names(list_vec: &Vec<(String, String, bool, bool)>) -> Vec<String> {
    let mut new_vec: Vec<String> = Vec::new();

    for i in list_vec.iter() {
        new_vec.push(i.0.to_string());
    }

    return new_vec;
}

// Sort list vector.
fn sort_list_vector(list_vec: &Vec<(String, String, bool, bool)>) -> Result<Vec<(String, String, bool, bool)>, io::Error> {
    if list_vec.len() == 0 {
        return Ok(list_vec.clone());
    }

    let list_vec_names = get_list_vector_names(&list_vec);

    let mut list_vec_nums: Vec<usize> = Vec::new();

    for i in list_vec_names.iter() {
        list_vec_nums.push(match usize_from_gen_name(i) {
            Ok(o) => o,
            Err(e) => return Err(e),
        });
    }

    list_vec_nums.sort();

    let mut new_vec: Vec<(String, String, bool, bool)> = Vec::new();

    for i in list_vec_nums.iter() {
        for j in list_vec.iter() {
            let j_num: usize = match usize_from_gen_name(j.0.as_str()) {
                Ok(o) => o,
                Err(e) => return Err(e),
            };

            if &j_num == i {
                new_vec.push(j.clone());
                break;
            }
        }
    }

    return Ok(new_vec);
}

// Get oldest generation name.
pub fn get_oldest() -> Result<usize, io::Error> {
    let gen_names = get_list_vector_names(&match sort_list_vector(&match list_with_no_calls() {
        Ok(o) => o,
        Err(e) => return Err(e),
    }) {
        Ok(o) => o,
        Err(e) => return Err(e),
    });

    if gen_names.len() == 0 {
        error!("Tried to call generation::get_oldest(), when there are no generations!");
        return Err(custom_error("Not enough generations!"));
    }

    let oldest_name = gen_names[0].to_string();

    let oldest_number: usize = match usize_from_gen_name(oldest_name.as_str()) {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    return Ok(oldest_number);
}

// Get the 'current' generation TOML file.
pub fn current_gen() -> Result<Path, io::Error> {
    let current = match get_current() {
        Ok(o) => o,
        Err(e) => return Err(e),
    };

    return Ok(places::gens().add_str(&current.to_string()).add_str("gen.toml"));
}
