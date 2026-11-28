//! The launcher's logic without a window, one module per C# static class (same names), ported line by line.
//! Platform paths sit behind `paths::Paths` so Linux can follow.

pub mod adding;
pub mod art_jobs;
pub mod config;
pub mod ctr3ds_saves;
pub mod discord;
pub mod ctr_header_fix;
pub mod custom_games;
pub mod emu3ds;
pub mod engines;
pub mod game_entry;
pub mod game_options;
pub mod if_dex;
pub mod library;
pub mod mod_index;
pub mod mods;
pub mod nds_extract;
pub mod nds_patch;
pub mod net;
pub mod paths;
pub mod pc_games;
pub mod places;
pub mod platinum_save;
pub mod play_prep;
pub mod proc;
pub mod recomp;
pub mod save_guard;
pub mod save_info;
pub mod save_sets;
pub mod saves;
pub mod submit;
pub mod test_hooks;
pub mod updater;
pub mod zips;
