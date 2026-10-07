#![allow(non_snake_case, unused, warnings)]
#![feature(proc_macro_hygiene)]

use once_cell::sync::{Lazy, OnceCell};
use serde_derive::Deserialize;
use smash::app::{lua_bind::*, utility, BattleObjectModuleAccessor};
use smash::lib::lua_const::*;
use std::{
    collections::HashMap,
    fs,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Mutex,
    },
};

// Each enabled mod folder in sd:/ultimate/mods/ may contain this file at its root.
const IDENTIFIER: &str = "config_scale.toml";
const MODS_DIR: &str = "sd:/ultimate/mods";

// ---------------------------------------------------------------------------
// Config format (config_scale.toml):
//
//   kind  = "pikachu"          # optional default fighter for entries below
//   slots = [80,81,82]         # optional default costumes; -1 = all costumes
//   scale_in_results = true    # optional; false = normal size on the results screen
//
//   [[scale]]
//   value = 1.2                # size MULTIPLIER: 1.0 = normal
//   # kind / kinds / slots can be set here too to override the defaults:
//   # kinds = ["pikachu", "pichu"]
//   # slots = [80,81]
//   # scale_in_results = false
//
// If several entries match the same fighter, the LAST one wins.
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ConfigToml {
    kind: Option<String>,
    slots: Option<Vec<i32>>,
    scale_in_results: Option<bool>,
    scale: Option<Vec<ScaleToml>>,
}

#[derive(Deserialize)]
struct ScaleToml {
    kind: Option<String>,
    kinds: Option<Vec<String>>,
    slots: Option<Vec<i32>>,
    scale_in_results: Option<bool>,
    value: f32,
}

struct Entry {
    kind: i32,
    slots: Vec<i32>,
    value: f32,
    scale_in_results: bool,
}

static ENTRIES: OnceCell<Vec<Entry>> = OnceCell::new();

// Per-fighter memory (keyed by the fighter's module accessor address): the scale
// we last applied. If the current scale differs from it, the game changed the
// size itself (spawn, mushrooms, etc.), so we re-apply our multiplier on top.
// Tiny offset added to every size we apply. It guarantees the size we set can never
// equal a size the game writes later (for example the game restoring 1.0 while we
// had set exactly 1.0), which would make us think the game changed nothing.
const MARK: f32 = 0.0002;
// How far the current size may differ from what we applied before we assume the game changed it.
const TOLERANCE: f32 = 0.00002;

static APPLIED: Lazy<Mutex<HashMap<usize, f32>>> = Lazy::new(|| Mutex::new(HashMap::new()));

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
    let default_in_results = data.scale_in_results.unwrap_or(true);
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
        let in_results = s.scale_in_results.unwrap_or(default_in_results);
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
                    out.push(Entry { kind, slots: slots.clone(), value: s.value, scale_in_results: in_results });
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

    // The game's own "are we on the results screen?" check.
    #[link_name = "\u{1}_ZN3app8lua_bind35FighterManager__is_result_mode_implEPNS_14FighterManagerE"]
    fn is_result_mode_impl(fighter_manager: *mut smash::app::FighterManager) -> bool;

    // Called constantly by fighter scripts, so we use it as a per-frame trigger.
    #[link_name = "\u{1}_ZN3app8lua_bind22PostureModule__lr_implEPNS_26BattleObjectModuleAccessorE"]
    fn lr_impl(boma: *mut BattleObjectModuleAccessor) -> f32;
}

unsafe fn wanted_scale(boma: *mut BattleObjectModuleAccessor) -> Option<(f32, bool)> {
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
            result = Some((e.value, e.scale_in_results)); // last match wins
        }
    }
    result
}

// The game's FighterManager, captured the first time the game calls is_result_mode.
static FIGHTER_MANAGER: AtomicUsize = AtomicUsize::new(0);
static LOGGED_RESULTS: AtomicBool = AtomicBool::new(false);

#[skyline::hook(replace = is_result_mode_impl)]
unsafe fn result_mode_hook(fighter_manager: *mut smash::app::FighterManager) -> bool {
    if !fighter_manager.is_null()
        && FIGHTER_MANAGER.swap(fighter_manager as usize, Ordering::Relaxed) == 0
    {
        println!("[char_scale] FighterManager captured");
    }
    original!()(fighter_manager)
}

// True on the results screen: the game says so, or the fighter is in a win/lose status.
unsafe fn in_results(boma: *mut BattleObjectModuleAccessor) -> bool {
    let fm = FIGHTER_MANAGER.load(Ordering::Relaxed);
    let by_manager = fm != 0 && is_result_mode_impl(fm as *mut smash::app::FighterManager);
    let status = StatusModule::status_kind(boma);
    let by_status = status == *FIGHTER_STATUS_KIND_WIN || status == *FIGHTER_STATUS_KIND_LOSE;
    let result = by_manager || by_status;
    if result && !LOGGED_RESULTS.swap(true, Ordering::Relaxed) {
        println!("[char_scale] Results screen detected (manager: {}, status: {})", by_manager, by_status);
    }
    result
}

#[skyline::hook(replace = lr_impl)]
unsafe fn lr_hook(boma: *mut BattleObjectModuleAccessor) -> f32 {
    if let Some((mult, scale_in_results)) = wanted_scale(boma) {
        let key = boma as usize;
        if !scale_in_results && in_results(boma) {
            // Results screen with scaling turned off: undo our size once, then leave it alone.
            let mut applied = APPLIED.lock().unwrap();
            if let Some(last) = applied.remove(&key) {
                if (scale_impl(boma) - last).abs() <= TOLERANCE {
                    set_scale_impl(boma, (last - MARK) / mult, false);
                }
            }
            return original!()(boma);
        }
        let current = scale_impl(boma);
        let mut applied = APPLIED.lock().unwrap();
        let game_changed_it = match applied.get(&key) {
            Some(last) => (current - *last).abs() > TOLERANCE,
            None => true,
        };
        if game_changed_it {
            // `current` is the size the game wants right now (normal, mushroom, ...).
            set_scale_impl(boma, current * mult + MARK, false);
            // Remember what the game actually stored, so we never re-multiply our own value.
            applied.insert(key, scale_impl(boma));
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
    skyline::install_hooks!(lr_hook, result_mode_hook);
    println!("[char_scale] Loaded");
}
