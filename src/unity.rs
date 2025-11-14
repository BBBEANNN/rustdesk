use std::ffi::{c_char, CString};
use std::sync::RwLock;

use hbb_common::log;
use scrap::ImageFormat;

pub type UnityVideoFrameCallback = Option<
    extern "C" fn(
        peer_id: *const c_char,
        display: u32,
        width: u32,
        height: u32,
        stride: u32,
        format: u32,
        buffer: *const u8,
        len: usize,
    ),
>;

lazy_static::lazy_static! {
    static ref VIDEO_FRAME_CALLBACK: RwLock<UnityVideoFrameCallback> = RwLock::new(None);
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_register_video_frame_callback(
    callback: UnityVideoFrameCallback,
) {
    let mut guard = VIDEO_FRAME_CALLBACK.write().unwrap();
    *guard = callback;
}

pub fn notify_video_frame(
    peer_id: &str,
    display: usize,
    width: usize,
    height: usize,
    stride_hint: usize,
    format: ImageFormat,
    buffer: &[u8],
) {
    let callback_opt = {
        let guard = VIDEO_FRAME_CALLBACK.read().unwrap();
        *guard
    };

    let Some(callback) = callback_opt else {
        return;
    };

    let c_peer_id = match CString::new(peer_id) {
        Ok(value) => value,
        Err(err) => {
            log::warn!("Failed to convert peer id to CString for Unity callback: {}", err);
            return;
        }
    };

    let mut stride = if height > 0 {
        buffer.len() / height
    } else {
        0
    };
    if stride == 0 {
        stride = width.saturating_mul(4);
    }
    if stride_hint > stride {
        stride = stride_hint;
    }
    let format = image_format_to_u32(format);

    unsafe {
        callback(
            c_peer_id.as_ptr(),
            display as u32,
            width as u32,
            height as u32,
            stride as u32,
            format,
            buffer.as_ptr(),
            buffer.len(),
        );
    }
}

fn image_format_to_u32(format: ImageFormat) -> u32 {
    match format {
        ImageFormat::Raw => 0,
        ImageFormat::ABGR => 1,
        ImageFormat::ARGB => 2,
    }
}
