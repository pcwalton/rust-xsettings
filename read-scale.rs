// Copyright 2016 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

extern crate xsettings;
extern crate x11_dl;

use std::ptr;
use std::str;
use x11_dl::xlib::Xlib;
use xsettings::Client;

pub fn main() {
    let display;
    let client;
    let xlib = Xlib::open().unwrap();
    unsafe {
        display = (xlib.XOpenDisplay)(ptr::null_mut());

        // Enumerate all properties.
        client = Client::new(display,
                             (xlib.XDefaultScreen)(display),
                             Box::new(|name, _, setting| {
                                 println!("{:?}={:?}", str::from_utf8(name), setting)
                             }),
                             Box::new(|_, _, _| {}));
    }

    // Print out a few well-known properties that describe the window scale.
    let gdk_unscaled_dpi: &[u8] = b"Gdk/UnscaledDPI";
    let gdk_xft_dpi: &[u8] = b"Xft/DPI";
    let gdk_window_scaling_factor: &[u8] = b"Gdk/WindowScalingFactor";
    for key in &[gdk_unscaled_dpi, gdk_xft_dpi, gdk_window_scaling_factor] {
        let key_str = str::from_utf8(key).unwrap();
        match client.get_setting(*key) {
            Err(err) => println!("{}: {:?}", key_str, err),
            Ok(setting) => println!("{}={:?}", key_str, setting),
        }
    }
}

