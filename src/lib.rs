#![allow(non_snake_case, unused, warnings)]
#![feature(proc_macro_hygiene)]

use once_cell::sync::OnceCell;
use serde_derive::Deserialize;
use smash::app::{lua_bind::*, utility, BattleObjectModuleAccessor};
use smash::lib::lua_const::*;
use std::{fs, path::Path};

// Each enabled mod folder in sd:/ultimate/mods/ may contain this file at its root.
const IDENTIFIER: &str = "config_scale.toml";
const MODS_DIR: &str = "sd:/ultimate/mods";

// ---------------------------------------------------------------------------
// Config format (config_scale.toml):
//
//   kind  = "pikachu"          # optional default fighter for entries below
//   slots = [80,81,82]         # optional default costumes; -1 = all costumes
//
//   [[scale]]
//   value = 1.2                # ABSOLUTE size: 1.0 = normal
//   # kind / kinds / slots can be set here too to override the defaults:
//   # kinds = ["pikachu", "pichu"]
//   # slots = [80,81]
//
// If several entries match the same fighter, the LAST one wins.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ConfigToml {
    kind: Option<String>,
    slots: Option<Vec<i32>>,
    scale: Option<Vec<ScaleToml>>,
}

#[derive(Deserialize)]
struct ScaleToml {
    kind: Option<String>,
    kinds: Option<Vec<String>>,
    slots: Option<Vec<i32>>,
    value: f32,
}

struct Entry {
    kind: i32,
    slots: Vec<i32>,
    value: f32,
}

static ENTRIES: OnceCell<Vec<Entry>> = OnceCell::new();

const FIGHTERS: &[(&str, i32)] = &[
    ("mario", 0),
    ("donkey", 1),
    ("link", 2),
    ("samus", 3),
    ("samusd", 4),
    ("yoshi", 5),
    ("kirby", 6),
    ("fox", 7),
    ("pikachu", 8),
    ("luigi", 9),
    ("ness", 10),
    ("captain", 11),
    ("purin", 12),
    ("peach", 13),
    ("daisy", 14),
    ("koopa", 15),
    ("sheik", 16),
    ("zelda", 17),
    ("mariod", 18),
    ("pichu", 19),
    ("falco", 20),
    ("marth", 21),
    ("lucina", 22),
    ("younglink", 23),
    ("ganon", 24),
    ("mewtwo", 25),
    ("roy", 26),
    ("chrom", 27),
    ("gamewatch", 28),
    ("metaknight", 29),
    ("pit", 30),
    ("pitb", 31),
    ("szerosuit", 32),
    ("wario", 33),
    ("snake", 34),
    ("ike", 35),
    ("pzenigame", 36),
    ("pfushigisou", 37),
    ("plizardon", 38),
    ("diddy", 39),
    ("lucas", 40),
    ("sonic", 41),
    ("dedede", 42),
    ("pikmin", 43),
    ("lucario", 44),
    ("robot", 45),
    ("toonlink", 46),
    ("wolf", 47),
    ("murabito", 48),
    ("rockman", 49),
    ("wiifit", 50),
    ("rosetta", 51),
    ("littlemac", 52),
    ("gekkouga", 53),
    ("palutena", 54),
    ("pacman", 55),
    ("reflet", 56),
    ("shulk", 57),
    ("koopajr", 58),
    ("duckhunt", 59),
    ("ryu", 60),
    ("ken", 61),
    ("cloud", 62),
    ("kamui", 63),
    ("bayonetta", 64),
    ("inkling", 65),
    ("ridley", 66),
    ("simon", 67),
    ("richter", 68),
    ("krool", 69),
    ("shizue", 70),
    ("gaogaen", 71),
    ("miifighter", 72),
    ("miiswordsman", 73),
    ("miigunner", 74),
    ("popo", 75),
    ("nana", 76),
    ("koopag", 77),
    ("packun", 81),
    ("jack", 82),
    ("brave", 83),
    ("buddy", 84),
    ("dolly", 85),
    ("master", 86),
    ("tantan", 87),
    ("pickel", 88),
    ("edge", 89),
    ("eflame", 90),
    ("elight", 91),
    ("demon", 92),
    ("trail", 93),
    ("ice_climber", 110),
    ("ptrainer", 114),
];

fn kind_from_name(name: &str) -> Option<i32> {
    let lower = name.to_lowercase();
    FIGHTERS.iter().find(|(n, _)| *n == lower).map(|(_, id)| *id)
}

fn read_config(path: &str, out: &mut Vec<Entry>) -> bool {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            println!("[char_scale] Could not read {}: {}", path, e);
            return false;
        }
    };
    let data: ConfigToml = match toml::from_str(&contents) {
        Ok(d) => d,
        Err(e) => {
            println!("[char_scale] Could not parse {}: {}", path, e);
            return false;
        }
    };
    let default_slots = data.slots.unwrap_or_default();
    let mut added = false;
    for s in data.scale.unwrap_or_default() {
        if !(s.value.is_finite() && s.value > 0.0) {
            println!("[char_scale] Ignoring invalid value {} in {}", s.value, path);
            continue;
        }
        let mut names: Vec<String> = Vec::new();
        if let Some(ks) = s.kinds {
            names.extend(ks);
        }
        if let Some(k) = s.kind {
            names.push(k);
        }
        if names.is_empty() {
            if let Some(k) = &data.kind {
                names.push(k.clone());
            }
        }
        if names.is_empty() {
            println!("[char_scale] Entry without a fighter in {}", path);
            continue;
        }
        let slots = match s.slots {
            Some(sl) => sl,
            None => default_slots.clone(),
        };
        if slots.is_empty() {
            println!("[char_scale] Entry without slots in {}", path);
            continue;
        }
        for name in names {
            match kind_from_name(&name) {
                Some(kind) => {
                    println!("[char_scale] {} slots {:?} -> scale {}", name, slots, s.value);
                    out.push(Entry { kind, slots: slots.clone(), value: s.value });
                    added = true;
                }
                None => println!("[char_scale] Unknown fighter '{}' in {}", name, path),
            }
        }
    }
    added
}

fn load_all() -> Vec<Entry> {
    let mut entries = Vec::new();
    let dir = match fs::read_dir(MODS_DIR) {
        Ok(d) => d,
        Err(e) => {
            println!("[char_scale] Could not read {}: {}", MODS_DIR, e);
            return entries;
        }
    };
    for entry in dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let mut path = entry.path();
        if !path.is_dir() {
            continue;
        }
        path.push(IDENTIFIER);
        if !Path::new(&path).exists() {
            continue;
        }
        path.pop();
        let folder = format!("{}", path.display());
        let enabled = arcropolis_api::is_mod_enabled(arcropolis_api::hash40(folder.as_str()).as_u64());
        if enabled {
            read_config(&format!("{}/{}", folder, IDENTIFIER), &mut entries);
        }
    }
    entries
}

extern "C" {
    #[link_name = "\u{1}_ZN3app8lua_bind29PostureModule__set_scale_implEPNS_26BattleObjectModuleAccessorEfb"]
    fn set_scale_impl(boma: *mut BattleObjectModuleAccessor, scale: f32, flag: bool);

    #[link_name = "\u{1}_ZN3app8lua_bind25PostureModule__scale_implEPNS_26BattleObjectModuleAccessorE"]
    fn scale_impl(boma: *mut BattleObjectModuleAccessor) -> f32;

    // Called constantly by fighter scripts, so we use it as a per-frame trigger.
    #[link_name = "\u{1}_ZN3app8lua_bind22PostureModule__lr_implEPNS_26BattleObjectModuleAccessorE"]
    fn lr_impl(boma: *mut BattleObjectModuleAccessor) -> f32;
}

unsafe fn wanted_scale(boma: *mut BattleObjectModuleAccessor) -> Option<f32> {
    let entries = ENTRIES.get()?;
    if boma.is_null() {
        return None;
    }
    if utility::get_category(&mut *boma) != *BATTLE_OBJECT_CATEGORY_FIGHTER {
        return None;
    }
    let kind = utility::get_kind(&mut *boma);
    let color = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_COLOR);
    let mut result = None;
    for e in entries {
        if e.kind == kind && (e.slots.contains(&-1) || e.slots.contains(&color)) {
            result = Some(e.value); // last match wins
        }
    }
    result
}

#[skyline::hook(replace = lr_impl)]
unsafe fn lr_hook(boma: *mut BattleObjectModuleAccessor) -> f32 {
    if let Some(target) = wanted_scale(boma) {
        let current = scale_impl(boma);
        if (current - target).abs() > 0.001 {
            set_scale_impl(boma, target, false);
        }
    }
    original!()(boma)
}

#[skyline::main(name = "char_scale")]
pub fn main() {
    println!("[char_scale] Loading...");
    let entries = std::thread::Builder::new()
        .stack_size(32 * 512 * 256)
        .spawn(load_all)
        .unwrap()
        .join()
        .unwrap_or_default();
    if entries.is_empty() {
        println!("[char_scale] No scale data found");
        return;
    }
    ENTRIES.set(entries).ok();
    skyline::install_hook!(lr_hook);
    println!("[char_scale] Loaded");
}
