use std::ptr;

use skyline::hook;
use skyline::libc::{c_char, c_void};

use log::info;

use crate::offsets::LOOKUP_STREAM_HASH_OFFSET;

#[hook(offset = LOOKUP_STREAM_HASH_OFFSET)]
fn lookup_by_stream_hash(out_path: *mut c_char, loaded_arc: *const c_void, size_out: *mut u64, offset_out: *mut u64, hash: u64) {
    match arcropolis_api::get_file_info(hash) {
        Some(arc_file) => {
            let path = match arcropolis_api::get_file_path(hash) {
                Some(path) => path,
                None => return original!()(out_path, loaded_arc, size_out, offset_out, hash)
            };

            unsafe {
                *size_out = arc_file.filesize as _;
                *offset_out = 0;
                info!("Loading '{}'...", path.display());
                let bytes = path.to_str().unwrap().as_bytes();
                ptr::copy_nonoverlapping(bytes.as_ptr(), out_path, bytes.len());
                *out_path.offset(bytes.len() as _) = 0u8;
            }
        },
        None => original!()(out_path, loaded_arc, size_out, offset_out, hash),
    }
}
