use std::{
    ffi::{c_char, c_void, CString},
    sync::RwLock,
};

use hbb_common::{log, ResultType};
use serde_json::json;

use super::{cstr_to_string, errno, plugins, str_to_cstr_ret, PluginReturn};

pub type UnityEventCallback =
    Option<extern "C" fn(event_type: *const c_char, payload: *const c_char)>;

lazy_static::lazy_static! {
    static ref EVENT_CALLBACK: RwLock<Option<extern "C" fn(event_type: *const c_char, payload: *const c_char)>> =
        RwLock::new(None);
}

fn make_error(code: i32, msg: &str) -> PluginReturn {
    PluginReturn::new(code, msg)
}

fn dispatch_from_result(result: ResultType<()>, context: &str) -> PluginReturn {
    match result {
        Ok(_) => PluginReturn::success(),
        Err(err) => make_error(errno::ERR_CALLBACK_FAILED, &format!("{}: {}", context, err)),
    }
}

fn dispatch_event(event_type: &str, payload: &str) {
    if let Some(callback) = *EVENT_CALLBACK.read().unwrap() {
        match (CString::new(event_type), CString::new(payload)) {
            (Ok(event_type), Ok(payload)) => unsafe {
                callback(event_type.as_ptr(), payload.as_ptr());
            },
            (Err(err), _) => {
                log::warn!(
                    "Failed to convert event type '{}' into CString: {}",
                    event_type,
                    err
                );
            }
            (_, Err(err)) => {
                log::warn!("Failed to convert Unity payload into CString: {}", err);
            }
        }
    }
}

fn get_id_and_peer<'a>(id: *const c_char, peer: *const c_char) -> ResultType<(String, String)> {
    let id = cstr_to_string(id)?;
    let peer = cstr_to_string(peer)?;
    Ok((id, peer))
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_register_event_callback(callback: UnityEventCallback) {
    let mut guard = EVENT_CALLBACK.write().unwrap();
    *guard = callback;
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_init_plugin_framework() -> PluginReturn {
    super::init();
    PluginReturn::success()
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_load_plugin(id: *const c_char) -> PluginReturn {
    match cstr_to_string(id) {
        Ok(id) => dispatch_from_result(super::load_plugin(&id), "Load plugin"),
        Err(err) => make_error(
            errno::ERR_CALLBACK_INVALID_ARGS,
            &format!("Invalid plugin id: {}", err),
        ),
    }
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_unload_plugin(id: *const c_char) -> PluginReturn {
    match cstr_to_string(id) {
        Ok(id) => dispatch_from_result(super::unload_plugin(&id), "Unload plugin"),
        Err(err) => make_error(
            errno::ERR_CALLBACK_INVALID_ARGS,
            &format!("Invalid plugin id: {}", err),
        ),
    }
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_reload_plugin(id: *const c_char) -> PluginReturn {
    match cstr_to_string(id) {
        Ok(id) => dispatch_from_result(super::reload_plugin(&id), "Reload plugin"),
        Err(err) => make_error(
            errno::ERR_CALLBACK_INVALID_ARGS,
            &format!("Invalid plugin id: {}", err),
        ),
    }
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_handle_ui_event(
    id: *const c_char,
    peer: *const c_char,
    event: *const c_void,
    len: usize,
) -> PluginReturn {
    let (id, peer) = match get_id_and_peer(id, peer) {
        Ok(v) => v,
        Err(err) => {
            return make_error(
                errno::ERR_CALLBACK_INVALID_ARGS,
                &format!("Invalid plugin arguments: {}", err),
            )
        }
    };
    let event_slice = unsafe { std::slice::from_raw_parts(event as *const u8, len) };
    dispatch_from_result(
        plugins::handle_ui_event(&id, &peer, event_slice),
        "Handle UI event",
    )
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_handle_server_event(
    id: *const c_char,
    peer: *const c_char,
    event: *const c_void,
    len: usize,
) -> PluginReturn {
    let (id, peer) = match get_id_and_peer(id, peer) {
        Ok(v) => v,
        Err(err) => {
            return make_error(
                errno::ERR_CALLBACK_INVALID_ARGS,
                &format!("Invalid plugin arguments: {}", err),
            )
        }
    };
    let event_slice = unsafe { std::slice::from_raw_parts(event as *const u8, len) };
    dispatch_from_result(
        plugins::handle_server_event(&id, &peer, event_slice),
        "Handle server event",
    )
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_get_plugins() -> *const c_char {
    let infos = plugins::get_plugin_infos();
    let guard = infos.read().unwrap();
    let payload = guard
        .values()
        .map(|info| {
            json!({
                "desc": info.desc.clone(),
                "path": info.path.clone(),
                "uninstalled": info.uninstalled,
            })
        })
        .collect::<Vec<_>>();
    let json = serde_json::to_string(&payload).unwrap_or_else(|err| {
        log::error!("Failed to serialize plugin descriptors: {}", err);
        "[]".to_string()
    });
    str_to_cstr_ret(&json)
}

#[no_mangle]
pub extern "C" fn rustdesk_unity_free(ptr: *mut c_void) {
    super::free_c_ptr(ptr);
}

pub(super) fn notify_manager_event(payload: &str) {
    dispatch_event(super::MSG_TO_UI_TYPE_PLUGIN_MANAGER, payload);
}

pub(super) fn notify_reload_event(payload: &str) {
    dispatch_event(super::MSG_TO_UI_TYPE_PLUGIN_RELOAD, payload);
}

pub(super) fn notify_option_event(payload: &str) {
    dispatch_event(super::MSG_TO_UI_TYPE_PLUGIN_OPTION, payload);
}

pub(super) fn notify_plugin_event(payload: &str) {
    dispatch_event(super::MSG_TO_UI_TYPE_PLUGIN_EVENT, payload);
}
