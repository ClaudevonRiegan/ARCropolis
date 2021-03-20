use std::{
    fs,
    collections::HashMap,
};

use crate::{config::CONFIG, fs::Metadata, runtime, visit::{Modpack, ModFile}};

use owo_colors::OwoColorize;

use smash_arc::{ArcLookup, FileData, FileDataFlags, FileInfoIndiceIdx, Hash40};

use runtime::{
    LoadedTables,
};

use log::warn;

type ArcCallback = extern "C" fn(Hash40, *mut skyline::libc::c_void, usize) -> bool;

lazy_static::lazy_static! {
    pub static ref MOD_FILES: parking_lot::RwLock<ModFiles> = parking_lot::RwLock::new(ModFiles::new());

    // For ResInflateThread
    pub static ref INCOMING_IDX: parking_lot::RwLock<Option<FileIndex>> = parking_lot::RwLock::new(None);
}

#[no_mangle]
pub extern "C" fn subscribe_callback(_hash: Hash40, _extension: *const u8, _extension_len: usize, _callback: ArcCallback) {
    // Deprecated
    warn!("{}", "Another plugin is trying to reach ARCropolis, but this API is deprecated.".red());
}

#[no_mangle]
pub extern "C" fn subscribe_callback_with_size(_hash: Hash40, _filesize: u32, _extension: *const u8, _extension_len: usize, _callback: ArcCallback) {
    // Deprecated
    warn!("{}", "Another plugin is trying to reach ARCropolis, but this API is deprecated.".red());
}

const REGIONS: &[&str] = &[
    "jp_ja",
    "us_en",
    "us_fr",
    "us_es",
    "eu_en",
    "eu_fr",
    "eu_es",
    "eu_de",
    "eu_nl",
    "eu_it",
    "eu_ru",
    "kr_ko",
    "zh_cn",
    "zh_tw",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileIndex {
    Regular(FileInfoIndiceIdx),
    Stream(Hash40),
}

#[repr(transparent)]
pub struct ModFiles(pub HashMap<FileIndex, FileCtx>);

#[derive(Debug, Clone)]
pub struct FileCtx {
    pub file: ModFile,
    pub hash: Hash40,
    pub orig_subfile: FileData,
    pub index: FileInfoIndiceIdx,
}

#[macro_export]
macro_rules! get_from_file_info_indice_index {
    ($index:expr) => {
        parking_lot::RwLockReadGuard::try_map(
            $crate::replacement_files::MOD_FILES.read(),
            |x| x.get(FileIndex::Regular($index))
        )
    };
}

impl ModFiles {
    fn new() -> Self {
        let instance = Self(HashMap::new());

        let config = CONFIG.read();
        
        let mut mods: Vec<Modpack> = vec![];

        // TODO: Build a cache using the timestamp of every Mod directory to confirm if something changed. If not, load everything and fill the tables without running a discovery

        if config.paths.arc.exists() {
            mods.push(crate::visit::discover(&config.paths.arc));
        }

        if config.paths.umm.exists() {
            mods.append(&mut crate::visit::umm_directories(&config.paths.umm));
        }

        if let Some(extra_paths) = &config.paths.extra_paths {
            for path in extra_paths {
                if path.exists() {
                    mods.append(&mut crate::visit::umm_directories(&path));
                }
            }
        }

        println!("Moving to process_mods");

        ModFiles::process_mods(&mods);

        instance
    }

    fn process_mods(modpacks: &[Modpack]) {
        let user_region = smash_arc::Region::from(get_region_id(CONFIG.read().misc.region.as_ref().unwrap()).unwrap() + 1);

        // TODO: Read the info.toml for every Mod instance if it exists, store the priority and then sort the vector
        println!("Starting to iterate");


        let _cunt: Vec<(FileIndex, FileCtx)> = modpacks.iter().map(|modpack| {
            let cock = modpack.merge();
            cock.iter().filter_map(|(hash, modfile)| {
                // Use a FileCtx until the system is fully reworked
                let mut filectx = FileCtx::new();
                //let hash = modpath.hash40().unwrap();
                //let modfile = ModFile::new(modpack.path().join(modpath.path()));
                println!("{}", modfile.path().display());

                if modfile.is_stream() {
                        filectx.file = modfile.clone();
                        filectx.hash = *hash;

                        warn!("[ARC::Patching] File '{}' added as a Stream", filectx.file.path().display().bright_yellow());
                        Some((FileIndex::Stream(filectx.hash), filectx))
                    }
                    else {
                        let arc = LoadedTables::get_arc_mut();

                        // Does the file exist in the FilePath table? If not, discard it.
                        match arc.get_file_path_index_from_hash(*hash) {
                            Ok(index) => {
                                let file_info = arc.get_file_info_from_path_index(index);

                                // Check if a file is regional.
                                if file_info.flags.is_regional() {
                                    // Check if the file has a regional indicator
                                    let region = match modfile.get_region() {
                                        Some(region) => {
                                            region
                                        }
                                        // No regional indicator, use the system's region as default (Why? Because by this point, it isn't storing the game's region yet)
                                        None => user_region,
                                    };
                
                                    // Check if the Region of a file matches with the game's. If not, discard it.
                                    if region != user_region {
                                        return None;
                                    }
                                }

                                filectx.file = modfile.clone();
                                filectx.hash = *hash;
                                filectx.index = file_info.file_info_indice_index;
                        
                                Some((FileIndex::Regular(filectx.index), filectx))
                            },
                            Err(_) => {
                                warn!("[ARC::Patching] File '{}' was not found in data.arc", modfile.as_smash_path().display().bright_yellow());
                                None
                            },
                        }
                    }
            }).collect::<Vec<(FileIndex, FileCtx)>>()
        }).flatten().collect();
    }

    pub fn get(&self, file_index: FileIndex) -> Option<&FileCtx> {
        self.0.get(&file_index)
    }
}

pub fn get_region_id(region: &str) -> Option<u32> {
    REGIONS
        .iter()
        .position(|x| x == &region)
        .map(|x| x as u32)
}

impl FileCtx {
    pub fn new() -> Self {
        FileCtx {
            file: ModFile::new(""),
            hash: Hash40(0),
            orig_subfile: FileData {
                offset_in_folder: 0,
                comp_size: 0,
                decomp_size: 0,
                flags: FileDataFlags::new()
                .with_compressed(false)
                .with_use_zstd(false)
                .with_unk(0),
            },
            index: FileInfoIndiceIdx(0),
        }
    }

    pub fn metadata(&self) -> Result<Metadata, String> {
        crate::fs::metadata(self.hash)
    }

    pub fn get_file_content(&self) -> Vec<u8> {
        // TODO: Add error handling in case the user deleted the file while running and reboot Smash if they did. But maybe this requires extract checks because of callbacks?
        fs::read(&self.file.path()).unwrap()
    }
}
