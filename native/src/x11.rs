//! Coordinate X11 resizing with the completed software-rendered frame.
use std::cell::RefCell;
use std::ffi::{c_int, c_ulong, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use x11_dl::{sync, xlib};

type Decode =
    unsafe extern "C" fn(*mut xlib::Display, *mut xlib::XEvent, *mut xlib::xEvent) -> c_int;
struct State {
    xlib: xlib::Xlib,
    sync: sync::Xext,
    display: *mut xlib::Display,
    window: c_ulong,
    counter: c_ulong,
    protocol: c_ulong,
    request: c_ulong,
    decode: Option<Decode>,
    pending: Option<sync::XSyncValue>,
}
thread_local! { static STATE: RefCell<Option<State>> = const { RefCell::new(None) }; }
static PENDING: AtomicBool = AtomicBool::new(false);

// Xlib calls this on the event-loop thread, before winit discards unknown
// ClientMessages. Preserve its normal decoder for keyboard, focus and close.
unsafe extern "C" fn decode(
    display: *mut xlib::Display,
    event: *mut xlib::XEvent,
    wire: *mut xlib::xEvent,
) -> c_int {
    STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        let Some(state) = state.as_mut() else {
            return 0;
        };
        let result = state
            .decode
            .map_or(0, |decode| unsafe { decode(display, event, wire) });
        if result != 0 && unsafe { (*event).get_type() } == xlib::ClientMessage {
            let message = unsafe { (*event).client_message };
            if message.window == state.window
                && message.message_type == state.protocol
                && message.data.get_long(0) as c_ulong == state.request
            {
                state.pending = Some(sync::XSyncValue {
                    hi: message.data.get_long(3) as i32,
                    lo: message.data.get_long(2) as u32,
                });
                PENDING.store(true, Ordering::Relaxed);
            }
        }
        result
    })
}

pub fn pending() -> bool {
    PENDING.load(Ordering::Relaxed)
}

/// Runs on the event-loop thread after Iced has presented the redraw that
/// emitted RedrawRequested. X11 drawing and counter updates share a connection.
pub fn frame_presented() {
    STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        if let Some(state) = state.as_mut() {
            if let Some(value) = state.pending.take() {
                unsafe {
                    (state.sync.XSyncSetCounter)(state.display, state.counter, value);
                    (state.xlib.XFlush)(state.display);
                }
            }
        }
        PENDING.store(false, Ordering::Relaxed);
    });
}

/// Preserve pixels and negotiate the standard X11 resize synchronization.
///
/// # Safety
/// The display and window must be live Xlib handles on their owning thread.
pub unsafe fn retain_contents(display: *mut c_void, window: c_ulong) {
    STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        if state.is_none() {
            let libraries =
                xlib::Xlib::open().and_then(|xlib| sync::Xext::open().map(|sync| (xlib, sync)));
            let Ok((xlib, sync)) = libraries else {
                eprintln!("Notes: could not load X11 resize synchronization");
                return;
            };
            let display = display.cast::<xlib::Display>();
            let (mut major, mut minor) = (3, 1);
            if unsafe { (sync.XSyncInitialize)(display, &mut major, &mut minor) } == 0 {
                return;
            }
            let protocol = unsafe { (xlib.XInternAtom)(display, c"WM_PROTOCOLS".as_ptr(), 0) };
            let request =
                unsafe { (xlib.XInternAtom)(display, c"_NET_WM_SYNC_REQUEST".as_ptr(), 0) };
            let property =
                unsafe { (xlib.XInternAtom)(display, c"_NET_WM_SYNC_REQUEST_COUNTER".as_ptr(), 0) };
            let counter =
                unsafe { (sync.XSyncCreateCounter)(display, sync::XSyncValue { hi: 0, lo: 0 }) };
            let mut protocols = std::ptr::null_mut();
            let mut count = 0;
            let mut list = vec![];
            unsafe {
                if (xlib.XGetWMProtocols)(display, window, &mut protocols, &mut count) != 0
                    && !protocols.is_null()
                {
                    list.extend_from_slice(std::slice::from_raw_parts(protocols, count as usize));
                    (xlib.XFree)(protocols.cast());
                }
                (xlib.XChangeProperty)(
                    display,
                    window,
                    property,
                    xlib::XA_CARDINAL,
                    32,
                    xlib::PropModeReplace,
                    (&counter as *const c_ulong).cast(),
                    1,
                );
                list.push(request);
                (xlib.XSetWMProtocols)(display, window, list.as_mut_ptr(), list.len() as c_int);
            }
            let previous =
                unsafe { (xlib.XESetWireToEvent)(display, xlib::ClientMessage, Some(decode)) };
            *state = Some(State {
                xlib,
                sync,
                display,
                window,
                counter,
                protocol,
                request,
                decode: previous,
                pending: None,
            });
        }
        if let Some(state) = state.as_ref() {
            // All fields are X11 scalar values; only the two masked fields apply.
            let mut attributes: xlib::XSetWindowAttributes = unsafe { std::mem::zeroed() };
            attributes.bit_gravity = xlib::NorthWestGravity;
            attributes.background_pixel = 0;
            unsafe {
                (state.xlib.XChangeWindowAttributes)(
                    state.display,
                    window,
                    xlib::CWBitGravity | xlib::CWBackPixel,
                    &mut attributes,
                );
                (state.xlib.XFlush)(state.display);
            }
        }
    });
}
