//! Keep the last painted pixels while an X11 window is resized.
use std::ffi::{c_int, c_long, c_ulong, c_void};

#[repr(C)]
#[derive(Default)]
struct Attributes {
    background_pixmap: c_ulong,
    background_pixel: c_ulong,
    border_pixmap: c_ulong,
    border_pixel: c_ulong,
    bit_gravity: c_int,
    win_gravity: c_int,
    backing_store: c_int,
    backing_planes: c_ulong,
    backing_pixel: c_ulong,
    save_under: c_int,
    event_mask: c_long,
    do_not_propagate_mask: c_long,
    override_redirect: c_int,
    colormap: c_ulong,
    cursor: c_ulong,
}

#[link(name = "libX11.so.6", kind = "dylib", modifiers = "+verbatim")]
unsafe extern "C" {
    fn XChangeWindowAttributes(
        display: *mut c_void,
        window: c_ulong,
        mask: c_ulong,
        attributes: *mut Attributes,
    ) -> c_int;
    fn XFlush(display: *mut c_void) -> c_int;
}

/// Preserve the old frame and fill newly exposed pixels with the app background.
///
/// # Safety
/// The display and window must be live Xlib handles, used on their owning thread.
pub unsafe fn retain_contents(display: *mut c_void, window: c_ulong, dark: bool) {
    let mut attributes = Attributes {
        bit_gravity: 1, // NorthWestGravity; the X11 default forgets the whole frame.
        background_pixel: if dark { 0x19191c } else { 0xfcfcfc },
        ..Attributes::default()
    };
    // CWBitGravity | CWBackPixel: leave all input and window-manager settings alone.
    unsafe {
        XChangeWindowAttributes(display, window, (1 << 4) | (1 << 1), &mut attributes);
        XFlush(display);
    }
}
