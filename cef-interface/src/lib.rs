use std::os::raw::c_char;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use cef_api::CefApi;
use cef_api::{cef_list_value_t, List};

const DLL_PROCESS_ATTACH: u32 = 1;
const DLL_PROCESS_DETACH: u32 = 0;

struct App {
    circle: bool,
    cef: CefApi,
    pressed: Instant,
    event_tx: Sender<(i32, i32)>,
    event_rx: Receiver<(i32, i32)>,
}

static mut APP: Option<App> = None;
const CEF_INTERFACE_BROWSER: u32 = 102;

#[no_mangle]
pub extern "C" fn cef_initialize() {
    std::thread::spawn(|| {
        initialize();
    });
}

fn initialize() {
    std::thread::sleep(Duration::from_secs(2));

    let cef = CefApi::wait_loading().expect("No client.dll");
    let (event_tx, event_rx) = std::sync::mpsc::channel();
    cef.subscribe("circle_click", circle_click);
    cef.subscribe("circle_closed", circle_closed);

    let app = App {
        circle: false,
        pressed: Instant::now(),
        cef,
        event_tx,
        event_rx,
    };

    unsafe {
        APP = Some(app);
    }
}

#[no_mangle]
pub extern "C" fn cef_samp_mainloop() {
    if let Some(app) = unsafe { APP.as_mut() } {
        if client_api::utils::is_key_pressed(0x72) {
            if app.pressed.elapsed() >= Duration::from_millis(500) {
                if !app.circle {
                    if app.cef.try_focus_browser(CEF_INTERFACE_BROWSER) {
                        let args = app.cef.create_list();
                        app.cef.hide_browser(CEF_INTERFACE_BROWSER, false);
                        app.cef.emit_event("show_actions", &args);

                        app.circle = true;
                    }
                } else {
                    app.cef.focus_browser(CEF_INTERFACE_BROWSER, false);
                    app.cef.hide_browser(CEF_INTERFACE_BROWSER, true);
                    app.circle = false;
                }

                app.pressed = Instant::now();
            }
        }

        while let Ok((x, y)) = app.event_rx.try_recv() {
            let x = x as f32;
            let y = y as f32;

            println!("got {} {}", x, y);

            let mut min = 10000.0f32;
            let mut min_id = u16::max_value();

            if let Some(mut players) = client_api::samp::players::players() {
                for player in players.filter(|p| p.is_in_stream()) {
                    if let Some(remote) = player.remote_player() {
                        let pos = remote.position();
                        let (p_x, p_y) = client_api::gta::display::calc_screen_coords(&pos)
                            .unwrap_or((-1.0, -1.0));

                        let delta = ((p_x - x).powf(2.0) + (p_y - y).powf(2.0)).sqrt();

                        if delta < min {
                            min = delta;
                            min_id = remote.id();
                        }
                    }
                }
            }

            println!("{} {}", min_id, min);

            if min <= 20.0 {
                if let Some(player) = client_api::samp::players::find_player(min_id as i32)
                    .as_ref()
                    .and_then(|p| p.name())
                {
                    let args = app.cef.create_list();
                    let name = cef::types::string::CefString::new(player);
                    args.set_string(0, &name);
                    args.set_integer(1, min_id as i32);
                    app.cef.emit_event("circle_found_player", &args);
                }
            }
        }
    }
}

pub extern "C" fn circle_click(event: *const c_char, args: *mut cef_list_value_t) -> i32 {
    println!("CIRCLE CLOCK !!!");
    if let Some(args) = List::try_from_raw(args) {
        if args.len() == 3 {
            let x = args.integer(1);
            let y = args.integer(2);

            println!("{} {}", x, y);

            unsafe {
                APP.as_mut().map(|app| app.event_tx.send((x, y)));
            }
        }
    }

    1
}

pub extern "C" fn circle_closed(_: *const c_char, _: *mut cef_list_value_t) -> i32 {
    if let Some(app) = unsafe { APP.as_mut() } {
        app.circle = false;
        println!("CLOSED");
    }

    1
}