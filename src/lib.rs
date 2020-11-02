#![feature(proc_macro_hygiene)]
#![feature(str_strip)]
#![feature(asm)]

use std::io::Write;
use std::ffi::CStr;
use std::net::IpAddr;

mod config;
use config::CONFIG;

mod hashes;
mod stream;

mod offsets;
use offsets::{ ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET, IDK_OFFSET, PARSE_EFF_NUTEXB_OFFSET, PARSE_EFF_OFFSET, PARSE_PARAM_OFFSET, PARSE_MODEL_XMB_OFFSET, PARSE_ARC_FILE_OFFSET, PARSE_FONT_FILE_OFFSET, PARSE_NUMSHB_FILE_OFFSET,PARSE_NUMATB_NUTEXB_OFFSET, PARSE_NUMSHEXB_FILE_OFFSET, PARSE_NUMATB_FILE_OFFSET, PARSE_NUMDLB_FILE_OFFSET, PARSE_LOG_XMB_OFFSET, PARSE_MODEL_XMB_2_OFFSET, TITLE_SCREEN_VERSION_OFFSET, PARSE_NUS3BANK_FILE_OFFSET };

mod logging;
use log::{ trace, info };

use skyline::nn;
use skyline::hooks::InlineCtx;
use skyline::{hook, install_hooks};

use smash::hash40;
use smash::resource::{FileState, LoadedTables, ResServiceState, Table2Entry};

use arcropolis_api as arc_api;
use arc_api::{ ArcInfo, ArcCallback, CallbackType };

use owo_colors::OwoColorize;

#[hook(offset = IDK_OFFSET)]
unsafe fn idk(res_state: *const ResServiceState, table1_idx: u32, flag_related: u32) {
    handle_file_load(table1_idx);
    original!()(res_state, table1_idx, flag_related);
}

#[hook(offset = ADD_IDX_TO_TABLE1_AND_TABLE2_OFFSET)]
unsafe fn add_idx_to_table1_and_table2(loaded_table: *const LoadedTables, table1_idx: u32) {
    handle_file_load(table1_idx);
    original!()(loaded_table, table1_idx);
}

#[hook(offset = PARSE_EFF_OFFSET, inline)]
fn parse_eff(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[10].w.as_ref());
    }
}

#[hook(offset = PARSE_PARAM_OFFSET, inline)]
fn parse_param_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[20].x.as_ref()) as *const u32));
    }
}

#[hook(offset = PARSE_MODEL_XMB_OFFSET, inline)]
fn parse_model_xmb(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[22].w.as_ref());
    }
}

#[hook(offset = PARSE_MODEL_XMB_2_OFFSET, inline)]
fn parse_model_xmb2(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[22].w.as_ref());
    }
}

#[hook(offset = PARSE_LOG_XMB_OFFSET, inline)]
fn parse_log_xmb(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[19].w.as_ref());
    }
}

#[hook(offset = PARSE_ARC_FILE_OFFSET, inline)]
fn parse_arc_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[8].w.as_ref());
    }
}

#[hook(offset = PARSE_FONT_FILE_OFFSET, inline)]
fn parse_font_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*((*ctx.registers[19].x.as_ref()) as *const u32));
    }
}

/// Causes a crash when finishing a battle with Dark Samus
#[hook(offset = PARSE_NUMDLB_FILE_OFFSET, inline)]
fn parse_numdlb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[1].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMSHEXB_FILE_OFFSET, inline)]
fn parse_numshexb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMATB_FILE_OFFSET, inline)]
fn parse_numatb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[23].w.as_ref());
    }
}

#[hook(offset = PARSE_EFF_NUTEXB_OFFSET, inline)]
fn parse_eff_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[24].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMATB_NUTEXB_OFFSET, inline)]
fn parse_numatb_nutexb(ctx: &InlineCtx) {
    unsafe {
        handle_texture_files(*ctx.registers[25].w.as_ref());
    }
}

#[hook(offset = PARSE_NUMSHB_FILE_OFFSET, inline)]
fn parse_numshb_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[24].w.as_ref());
    }
}

#[hook(offset = PARSE_NUS3BANK_FILE_OFFSET, inline)]
fn parse_nus3bank_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[8].w.as_ref());
    }
}

// 9.0.1
#[hook(offset = 0x35bae10, inline)]
fn parse_bntx_file(ctx: &InlineCtx) {
    unsafe {
        handle_file_overwrite(*ctx.registers[9].w.as_ref());
    }
}

fn get_arcinfo_by_t1index<'a>(table1_idx: u32) -> Option<(ArcInfo, &'a mut Table2Entry)> {
    let loaded_tables = LoadedTables::get_instance();
    let hash = loaded_tables.get_hash_from_t1_index(table1_idx).as_u64();

    let table2entry = match loaded_tables.get_t2_mut(table1_idx) {
        Ok(entry) => entry,
        Err(_) => {
            return None;
        }
    };

    trace!("[ARC::Loading | #{}] File: {}, Hash: {}, Status: {}", table1_idx.green(), hashes::get(hash).unwrap_or(&"Unknown").bright_yellow(), hash.cyan(), table2entry.bright_magenta());

    match arcropolis_api::get_file_info(hash) {
        Some(arc_info) => {
            info!("[ARC::Loading | #{}] Hash matching for file: '{}'", table1_idx.green(), hashes::get(hash).unwrap_or(&"Unknown").bright_yellow());
            Some((*arc_info, table2entry))
        },
        None => None,
    }
}

fn handle_file_load(table1_idx: u32) {
    // Println!() calls are on purpose so these show up no matter what.
    if let Some((arc_file, table2entry)) = get_arcinfo_by_t1index(table1_idx) {
        // Some formats don't appreciate me replacing the data pointer
        if !is_extension_allowed(arc_file.extension) {
            return;
        }

        if table2entry.state == FileState::Loaded {
            // For files that are too dependent on timing, make sure the pointer is overwritten instead of swapped
            if [hash40("nusktb"), hash40("bin"), hash40("numdlb")].contains(&arc_file.extension) {
                // Let handle_file_overwrite take care of the rest
                handle_file_overwrite(table1_idx);
                return;
            }
        }

        info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(arc_file.hash).unwrap_or(&"Unknown").bright_yellow());

        // Get the file's content as a &mut [u8]
        let data = match arcropolis_api::get_file_content(arc_file.hash) {
            Some(content) => content,
            // Either the file had an accident at some point (deletion?) or we're dealing with a listening callback
            None => return
        };

        unsafe {
            if !table2entry.data.is_null() {
                skyline::libc::free(table2entry.data as *const skyline::libc::c_void);
            }
        }

        // Replace the pointer to the original file with one pointing to the file's content 
        table2entry.data = Box::leak(data).as_ptr();
        // Mark this file as loaded
        table2entry.state = FileState::Loaded;
        table2entry.flags = 43;
    }
}

fn handle_file_overwrite(table1_idx: u32) {
    if let Some((arc_info, table2entry)) = get_arcinfo_by_t1index(table1_idx) {
        if table2entry.state != FileState::Loaded {
            return;
        }

        info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(arc_info.hash).unwrap_or(&"Unknown").bright_yellow());

        // Get the file's content as a &mut [u8]
        let data = match arcropolis_api::get_file_content(arc_info.hash) {
            Some(content) => content,
            // Either the file had an accident at some point (deletion?) or we're dealing with a listening callback
            None => return
        };

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, data.len());
            data_slice.write(&data).unwrap();
        }
    }
}

fn handle_texture_files(table1_idx: u32) {
    if let Some((arc_info, table2entry)) = get_arcinfo_by_t1index(table1_idx) {
        if table2entry.state != FileState::Loaded {
            return;
        }

        let old_size = arc_info.orig_decomp_size as usize;
        let new_size = arc_info.filesize as usize;

        // Get the file's content as a &mut [u8]
        let data = match arcropolis_api::get_file_content(arc_info.hash) {
            Some(content) => {
                info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(arc_info.hash).unwrap_or(&"Unknown").bright_yellow());
                content
            },
            // Either the file had an accident at some point (deletion?) or we're dealing with a listening callback
            None => {
                if new_size > old_size {
                    // Make a slice out of the original texture if the patched size is larger so we can hack it back to a working state
                    unsafe { Box::from_raw(std::slice::from_raw_parts_mut(table2entry.data as *mut u8, old_size) as *mut [u8]) }
                } else {
                    // The size is the same, no fixing to perform, we can let the game do its own thing.
                    return;
                }
            }
        };

        unsafe {
            let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, new_size);

            // If the patched size is larger than the original one, some fixing is required
            if new_size > old_size {
                // Copy the content at the beginning
                data_slice[0..new_size - 0xB0].copy_from_slice(&data[0..data.len() - 0xB0]);
                // Copy our footer at the end
                data_slice[new_size - 0xB0..new_size].copy_from_slice(&data[data.len() - 0xB0..data.len()]);
            } else {
                data_slice.write(&data).unwrap();
            }
        }
    }
}

// fn handle_texture_files(table1_idx: u32) {
//     if let Some((arc_file, table2entry)) = get_arcfile_by_t1index(table1_idx) {
//         if table2entry.state != FileState::Loaded {
//             return;
//         }

//         let old_size = arc_file.orig_decomp_size as usize;
//         let new_size = arc_file.filesize as usize;

//         // Get the file's content as a &mut [u8]
//         let data = match arcropolis_api::get_file_content(arc_file.hash) {
//             Some(content) => {
//                 info!("[ARC::Replace | #{}] Replacing '{}'", table1_idx.green(), hashes::get(arc_file.hash).unwrap_or(&"Unknown").bright_yellow());
//                 content
//             },
//             // Either the file had an accident at some point (deletion?) or we're dealing with a listening callback
//             None => {
//                 if new_size > old_size {
//                     // Make a slice out of the original texture if the patched size is larger so we can hack it back to a working state
//                     unsafe { Box::from_raw(std::slice::from_raw_parts_mut(table2entry.data as *mut u8, old_size) as *mut [u8]) }
//                 } else {
//                     // The size is the same, no fixing to perform, we can let the game do its own thing.
//                     return;
//                 }
//             }
//         };

//         unsafe {
//             let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, new_size);

//             // If the patched size is larger than the original one, some fixing is required
//             if new_size > old_size {
//                 // Copy the content at the beginning
//                 data_slice[0..new_size - 0xB0].copy_from_slice(&data[0..data.len() - 0xB0]);
//                 // Copy our footer at the end
//                 data_slice[new_size - 0xB0..new_size].copy_from_slice(&data[data.len() - 0xB0..data.len()]);
//             } else {
//                 data_slice.write(&data).unwrap();
//             }
//         }
//     }
// }

#[hook(offset = TITLE_SCREEN_VERSION_OFFSET)]
fn change_version_string(arg1: u64, string: *const u8) {
    unsafe {
        // Convert the string passed to the function to a &str
        let original_str = CStr::from_ptr(string as _).to_str().unwrap();

        // If it contains "Ver.", this is what we're looking for
        if original_str.contains("Ver.") {
            // Build a new version string with ARCropolis' included
            let new_str = format!(
                "Smash {}\nARCropolis Ver. {}\0",
                original_str,
                env!("CARGO_PKG_VERSION").to_string()
            );
            // Return our version string instead of the original
            original!()(arg1, skyline::c_str(&new_str))
        } else {
            // Just call the original
            original!()(arg1, string)
        }
    }
}

pub fn is_extension_allowed(extension: u64) -> bool {
    // Check extensions
    ![hash40("numshb"), hash40("nutexb"), hash40("eff"), hash40("prc"), hash40("stprm"), hash40("stdat"), hash40("xmb"), hash40("arc"), hash40("bfotf"), hash40("bfttf"), hash40("numatb"), hash40("numshexb"), hash40("nus3bank"), hash40("bntx")].contains(&extension)
}

#[hook(offset = 0x2c5994, inline)]
fn initial_loading(_ctx: &InlineCtx) {
    // Initialize logger
    logging::init(CONFIG.logger.as_ref().unwrap().logger_level.into()).unwrap();

    // TODO: Modpack selector menu here if a key is held

    // Discover files
    unsafe {
        nn::oe::SetCpuBoostMode(nn::oe::CpuBoostMode::Boost);

        arcropolis_api::discover_files(&CONFIG.paths.arc, false);
        arcropolis_api::discover_files(&CONFIG.paths.umm, true);

        println!("Finished discovery");

        // Extension listener CB
        //arcropolis_api::register_callback(CallbackType::Extension(hash40("msbt")), ArcCallback::Listener(extension_listener_callback));
        // Extension editor CB
        arcropolis_api::register_callback(CallbackType::Extension(hash40("nutexb")), ArcCallback::Editor(0, nutexb_editor_callback));
        // File listening CB
        arcropolis_api::register_callback(CallbackType::File(hash40("ui/layout/system/loading/loading/layout.arc")), ArcCallback::Listener(file_listener_callback));
        // File editor CB
        arcropolis_api::register_callback(CallbackType::File(hash40("ui/layout/system/loading/loading/layout.arc")), ArcCallback::Editor(0, file_editor_callback));

        nn::oe::SetCpuBoostMode(nn::oe::CpuBoostMode::Disabled);
    }
}

// Callback Examples

extern "C" fn file_listener_callback(hash: u64) {
    println!("File listener reached for hash {}", hash);
}

extern "C" fn file_editor_callback(infos: *const ArcInfo, _data: *mut arc_api::ArcFile) -> bool {
    unsafe {
        let infos = *infos;
        println!("File_editor infos: {:?}", infos);
    }

    false
}

// Need to provide a way to get the table2entry's data pointer for that
extern "C" fn nutexb_editor_callback(infos: *const ArcInfo, _data: *mut arc_api::ArcFile) -> bool {
        // let arc_file = *infos;

        // let old_size = arc_file.orig_decomp_size as usize;
        // let new_size = arc_file.filesize as usize;

        // unsafe {
        //     if new_size > (*data).len as _ {
        //         let mut data_slice = std::slice::from_raw_parts_mut(table2entry.data as *mut u8, new_size);

        //         // If the patched size is larger than the original one, some fixing is required
        //         if new_size > old_size {
        //             // Copy the content at the beginning
        //             data_slice[0..new_size - 0xB0].copy_from_slice(&data[0..data.len() - 0xB0]);
        //             // Copy our footer at the end
        //             data_slice[new_size - 0xB0..new_size].copy_from_slice(&data[data.len() - 0xB0..data.len()]);
        //         } else {
        //             data_slice.write(&data).unwrap();
        //         }
        //     }
        // }

    println!("Nutexb_editor infos: {:?}", infos);

    false
}

#[skyline::main(name = "arcropolis")]
pub fn main() {
    // Check if an update is available
    if skyline_update::check_update(IpAddr::V4(CONFIG.updater.as_ref().unwrap().server_ip), "ARCropolis", env!("CARGO_PKG_VERSION"), CONFIG.updater.as_ref().unwrap().beta_updates) {
        skyline::nn::oe::RestartProgramNoArgs();
    }

    // Load hashes from rom:/skyline/hashes.txt if the file is present
    hashes::init();
    // Look for the offset of the various functions to hook
    offsets::search_offsets();

    install_hooks!(
        idk,
        add_idx_to_table1_and_table2,
        stream::lookup_by_stream_hash,
        parse_eff_nutexb,
        parse_eff,
        parse_param_file,
        parse_model_xmb,
        parse_model_xmb2,
        parse_log_xmb,
        parse_arc_file,
        parse_font_file,
        //parse_numdlb_file,
        parse_numshb_file,
        parse_numshexb_file,
        parse_numatb_file,
        parse_numatb_nutexb,
        parse_bntx_file,
        parse_nus3bank_file,
        change_version_string,
        initial_loading,
    );

    println!(
        "ARCropolis v{} - File replacement plugin is now installed",
        env!("CARGO_PKG_VERSION")
    );
}
