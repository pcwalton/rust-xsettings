// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate libc;
extern crate x11_dl;

use libc::{c_char, c_int, c_long, c_ulong, c_ushort, c_void};
use std::ffi::{CStr, CString};
use std::fmt::{self, Debug, Formatter};
use std::marker::PhantomData;
use std::mem;
use std::ptr;
use x11_dl::xlib::{Bool, Display, False, Window, XEvent};

pub use self::XSettingsResult as Error;

pub type XSettingsNotifyFunc = unsafe extern "C" fn(name: *const c_char,
                                                    action: XSettingsAction,
                                                    setting: *mut XSettingsSetting,
                                                    cb_data: *mut c_void);

pub type XSettingsWatchFunc = unsafe extern "C" fn(window: Window,
                                                   is_start: Bool,
                                                   mask: c_long,
                                                   cb_data: *mut c_void);

pub type NotifyFunc = Box<for<'a> FnMut(&[u8], XSettingsAction, SettingRef<'a>)>;

pub type WatchFunc = Box<FnMut(Window, bool, c_long)>;

#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum XSettingsAction {
    New = 0,
    Changed = 1,
    Deleted = 2,
}

#[derive(Copy, Clone, Debug)]
#[repr(u32)]
pub enum XSettingsType {
    Int = 0,
    String = 1,
    Color = 2,
    None = 0xff,
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u32)]
pub enum XSettingsResult {
    Success = 0,
    NoMem = 1,
    Access = 2,
    Failed = 3,
    NoEntry = 4,
    DuplicateEntry = 5,
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(C)]
pub struct XSettingsColor {
    red: c_ushort,
    green: c_ushort,
    blue: c_ushort,
    alpha: c_ushort,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct XSettingsSetting {
    name: *const c_char,
    setting_type: XSettingsType,
    data: u64,
    last_change_serial: c_ulong,
}

#[derive(Copy, Clone, Debug)]
pub enum SettingData<'a> {
    Int(c_int),
    String(&'a [u8]),
    Color(XSettingsColor),
    None,
}

impl<'a> SettingData<'a> {
    unsafe fn from_raw(setting: *mut XSettingsSetting) -> SettingData<'a> {
        match (*setting).setting_type {
            XSettingsType::Int => {
                SettingData::Int(*mem::transmute::<_,*const c_int>(&(*setting).data))
            }
            XSettingsType::String => {
                let string = CStr::from_ptr(mem::transmute::<_,*const c_char>((*setting).data));
                SettingData::String(string.to_bytes())
            }
            XSettingsType::Color => {
                SettingData::Color(*mem::transmute::<_,*const XSettingsColor>(&(*setting).data))
            }
            XSettingsType::None => SettingData::None,
        }
    }
}

pub struct Setting {
    setting: *mut XSettingsSetting,
}

impl Debug for Setting {
    fn fmt(&self, f: &mut Formatter) -> Result<(),fmt::Error> {
        self.data().fmt(f)
    }
}

impl Drop for Setting {
    fn drop(&mut self) {
        unsafe {
            xsettings_setting_free(self.setting)
        }
    }
}

impl Clone for Setting {
    fn clone(&self) -> Setting {
        unsafe {
            Setting::from_raw(xsettings_setting_copy(self.setting))
        }
    }
}

impl PartialEq for Setting {
    fn eq(&self, other: &Setting) -> bool {
        unsafe {
            xsettings_setting_equal(self.setting, other.setting) != 0
        }
    }
}

impl Setting {
    pub unsafe fn from_raw(setting: *mut XSettingsSetting) -> Setting {
        Setting {
            setting: setting,
        }
    }

    pub fn data<'a>(&'a self) -> SettingData<'a> {
        unsafe {
            SettingData::from_raw(self.setting)
        }
    }
}

#[derive(Copy, Clone)]
pub struct SettingRef<'a> {
    setting: *mut XSettingsSetting,
    phantom: PhantomData<&'a mut XSettingsSetting>,
}

impl<'a> Debug for SettingRef<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(),fmt::Error> {
        self.data().fmt(f)
    }
}

impl<'a> SettingRef<'a> {
    pub unsafe fn from_raw(setting: *mut XSettingsSetting) -> SettingRef<'a> {
        SettingRef {
            setting: setting,
            phantom: PhantomData,
        }
    }

    pub fn data(&self) -> SettingData<'a> {
        unsafe {
            SettingData::from_raw(self.setting)
        }
    }
}

struct Callbacks {
    notify: NotifyFunc,
    watch: WatchFunc,
}

unsafe extern "C" fn notify_func(name: *const c_char,
                                 action: XSettingsAction,
                                 setting: *mut XSettingsSetting,
                                 cb_data: *mut c_void) {
    let callbacks: *mut Callbacks = mem::transmute(cb_data);
    let name = CStr::from_ptr(name);
    ((*callbacks).notify)(name.to_bytes(), action, SettingRef::from_raw(setting))
}

unsafe extern "C" fn watch_func(window: Window,
                                is_start: Bool,
                                mask: c_long,
                                cb_data: *mut c_void) {
    let callbacks: *mut Callbacks = mem::transmute(cb_data);
    ((*callbacks).watch)(window, is_start != False, mask)
}

#[repr(C)]
pub struct XSettingsClient {
    _private: c_int,
}

pub struct Client {
    client: *mut XSettingsClient,
    #[allow(dead_code)]
    callbacks: Box<Callbacks>,
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            xsettings_client_destroy(self.client)
        }
    }
}

impl Client {
    pub unsafe fn new(display: *mut Display, screen: c_int, notify: NotifyFunc, watch: WatchFunc)
                      -> Client {
        let mut callbacks = Box::new(Callbacks {
            notify: notify,
            watch: watch,
        });
        let client = xsettings_client_new(
            display,
            screen,
            notify_func,
            watch_func,
            mem::transmute::<&mut Callbacks,*mut c_void>(&mut *callbacks));
        Client {
            client: client,
            callbacks: callbacks,
        }
    }

    pub fn process_event(&mut self, event: &XEvent) -> bool {
        unsafe {
            xsettings_client_process_event(self.client, event) != False
        }
    }

    pub fn get_setting(&self, name: &[u8]) -> Result<Setting,Error> {
        let name = CString::new(name).expect("name() must be a valid C string!");
        let mut setting = ptr::null_mut();
        unsafe {
            let result = xsettings_client_get_setting(self.client, name.as_ptr(), &mut setting);
            if result == XSettingsResult::Success {
                Ok(Setting::from_raw(setting))
            } else {
                Err(result)
            }
        }
    }
}

#[link(name = "Xsettings-client")]
extern {
    fn xsettings_setting_copy(setting: *mut XSettingsSetting) -> *mut XSettingsSetting;
    fn xsettings_setting_free(setting: *mut XSettingsSetting);
    fn xsettings_setting_equal(setting_a: *mut XSettingsSetting, setting_b: *mut XSettingsSetting)
                               -> c_int;

    fn xsettings_client_new(display: *mut Display,
                            screen: c_int,
                            notify: XSettingsNotifyFunc,
                            watch: XSettingsWatchFunc,
                            cb_data: *mut c_void)
                            -> *mut XSettingsClient;
    fn xsettings_client_destroy(client: *mut XSettingsClient);
    fn xsettings_client_process_event(client: *mut XSettingsClient,
                                      event: *const XEvent) -> Bool;
    fn xsettings_client_get_setting(client: *mut XSettingsClient,
                                    name: *const c_char,
                                    setting: *mut *mut XSettingsSetting)
                                    -> XSettingsResult;
}

