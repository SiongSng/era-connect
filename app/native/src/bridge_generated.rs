#![allow(
    non_camel_case_types,
    unused,
    clippy::redundant_closure,
    clippy::useless_conversion,
    clippy::unit_arg,
    clippy::double_parens,
    non_snake_case,
    clippy::too_many_arguments
)]
// AUTO GENERATED FILE, DO NOT EDIT.
// Generated by `flutter_rust_bridge`@ 1.80.1.

use crate::api::*;
use core::panic::UnwindSafe;
use flutter_rust_bridge::rust2dart::IntoIntoDart;
use flutter_rust_bridge::*;
use std::ffi::c_void;
use std::sync::Arc;

// Section: imports

// Section: wire functions

fn wire_setup_logger_impl(port_: MessagePort) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, ()>(
        WrapInfo {
            debug_name: "setup_logger",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| setup_logger(),
    )
}
fn wire_download_vanilla_impl(port_: MessagePort) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, ()>(
        WrapInfo {
            debug_name: "download_vanilla",
            port: Some(port_),
            mode: FfiCallMode::Stream,
        },
        move || move |task_callback| download_vanilla(task_callback.stream_sink::<_, Progress>()),
    )
}
fn wire_launch_vanilla_impl(port_: MessagePort) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, ()>(
        WrapInfo {
            debug_name: "launch_vanilla",
            port: Some(port_),
            mode: FfiCallMode::Stream,
        },
        move || move |task_callback| launch_vanilla(task_callback.stream_sink::<_, Progress>()),
    )
}
fn wire_launch_forge_impl(port_: MessagePort) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, ()>(
        WrapInfo {
            debug_name: "launch_forge",
            port: Some(port_),
            mode: FfiCallMode::Stream,
        },
        move || move |task_callback| launch_forge(task_callback.stream_sink::<_, Progress>()),
    )
}
fn wire_launch_quilt_impl(port_: MessagePort) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, ()>(
        WrapInfo {
            debug_name: "launch_quilt",
            port: Some(port_),
            mode: FfiCallMode::Stream,
        },
        move || move |task_callback| launch_quilt(task_callback.stream_sink::<_, Progress>()),
    )
}
fn wire_fetch_state_impl(port_: MessagePort) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, DownloadState>(
        WrapInfo {
            debug_name: "fetch_state",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || move |task_callback| Ok(fetch_state()),
    )
}
fn wire_write_state_impl(port_: MessagePort, s: impl Wire2Api<DownloadState> + UnwindSafe) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, ()>(
        WrapInfo {
            debug_name: "write_state",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_s = s.wire2api();
            move |task_callback| Ok(write_state(api_s))
        },
    )
}
fn wire_get_ui_layout_config_impl(key: impl Wire2Api<Key> + UnwindSafe) -> support::WireSyncReturn {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap_sync(
        WrapInfo {
            debug_name: "get_ui_layout_config",
            port: None,
            mode: FfiCallMode::Sync,
        },
        move || {
            let api_key = key.wire2api();
            Ok(get_ui_layout_config(api_key))
        },
    )
}
fn wire_set_ui_layout_config_impl(port_: MessagePort, value: impl Wire2Api<Value> + UnwindSafe) {
    FLUTTER_RUST_BRIDGE_HANDLER.wrap::<_, _, _, ()>(
        WrapInfo {
            debug_name: "set_ui_layout_config",
            port: Some(port_),
            mode: FfiCallMode::Normal,
        },
        move || {
            let api_value = value.wire2api();
            move |task_callback| set_ui_layout_config(api_value)
        },
    )
}
// Section: wrapper structs

// Section: static checks

// Section: allocate functions

// Section: related functions

// Section: impl Wire2Api

pub trait Wire2Api<T> {
    fn wire2api(self) -> T;
}

impl<T, S> Wire2Api<Option<T>> for *mut S
where
    *mut S: Wire2Api<T>,
{
    fn wire2api(self) -> Option<T> {
        (!self.is_null()).then(|| self.wire2api())
    }
}
impl Wire2Api<bool> for bool {
    fn wire2api(self) -> bool {
        self
    }
}

impl Wire2Api<DownloadState> for i32 {
    fn wire2api(self) -> DownloadState {
        match self {
            0 => DownloadState::Downloading,
            1 => DownloadState::Paused,
            2 => DownloadState::Stopped,
            _ => unreachable!("Invalid variant for DownloadState: {}", self),
        }
    }
}
impl Wire2Api<i32> for i32 {
    fn wire2api(self) -> i32 {
        self
    }
}
impl Wire2Api<Key> for i32 {
    fn wire2api(self) -> Key {
        match self {
            0 => Key::CompletedSetup,
            _ => unreachable!("Invalid variant for Key: {}", self),
        }
    }
}

// Section: impl IntoDart

impl support::IntoDart for DownloadState {
    fn into_dart(self) -> support::DartAbi {
        match self {
            Self::Downloading => 0,
            Self::Paused => 1,
            Self::Stopped => 2,
        }
        .into_dart()
    }
}
impl support::IntoDartExceptPrimitive for DownloadState {}
impl rust2dart::IntoIntoDart<DownloadState> for DownloadState {
    fn into_into_dart(self) -> Self {
        self
    }
}

impl support::IntoDart for Progress {
    fn into_dart(self) -> support::DartAbi {
        vec![
            self.speed.into_into_dart().into_dart(),
            self.percentages.into_into_dart().into_dart(),
            self.current_size.into_into_dart().into_dart(),
            self.total_size.into_into_dart().into_dart(),
        ]
        .into_dart()
    }
}
impl support::IntoDartExceptPrimitive for Progress {}
impl rust2dart::IntoIntoDart<Progress> for Progress {
    fn into_into_dart(self) -> Self {
        self
    }
}

impl support::IntoDart for Value {
    fn into_dart(self) -> support::DartAbi {
        match self {
            Self::CompletedSetup(field0) => {
                vec![0.into_dart(), field0.into_into_dart().into_dart()]
            }
        }
        .into_dart()
    }
}
impl support::IntoDartExceptPrimitive for Value {}
impl rust2dart::IntoIntoDart<Value> for Value {
    fn into_into_dart(self) -> Self {
        self
    }
}

// Section: executor

support::lazy_static! {
    pub static ref FLUTTER_RUST_BRIDGE_HANDLER: support::DefaultHandler = Default::default();
}

#[cfg(not(target_family = "wasm"))]
#[path = "bridge_generated.io.rs"]
mod io;
#[cfg(not(target_family = "wasm"))]
pub use io::*;
