//! Preserve painted pixels when a composited X11 window changes size.
use std::ffi::{c_ulong, c_void};
use x11_dl::xlib;

/// Keep the previous frame; new surface pixels stay transparent until painted.
///
/// # Safety
/// The display and window must be live Xlib handles on their owning thread.
pub unsafe fn retain_contents(display: *mut c_void, window: c_ulong) {
    let Ok(xlib) = xlib::Xlib::open() else {
        eprintln!("Notes: could not initialize the X11 resize surface");
        return;
    };
    // All fields are X11 scalar values; only the two masked fields apply.
    let mut attributes: xlib::XSetWindowAttributes = unsafe { std::mem::zeroed() };
    attributes.bit_gravity = xlib::NorthWestGravity;
    attributes.background_pixel = 0;
    unsafe {
        (xlib.XChangeWindowAttributes)(
            display.cast(),
            window,
            xlib::CWBitGravity | xlib::CWBackPixel,
            &mut attributes,
        );
        (xlib.XFlush)(display.cast());
    }
}
