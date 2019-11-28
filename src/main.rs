use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
enum Animation {
    Grow,
    Shrink,
}

#[derive(Serialize, Deserialize)]
struct IndicatorConfig {
    max_size: u16,        // display pixels
    duration: u16,        // milliseconds
    thickness: u32,       // display pixels
    no_of_circles: u16,   // number of circles to display
    color: u32,           // color in hex, eg.: 0x00FECA
    animation: Animation, // 'Grow' | 'Shrink'
}

// sane defaults
impl std::default::Default for IndicatorConfig {
    fn default() -> IndicatorConfig {
        IndicatorConfig {
            max_size: 200u16,
            duration: 500u16,
            thickness: 1,
            no_of_circles: 5,
            color: 0xFFFFFF,
            animation: Animation::Grow,
        }
    }
}

fn main() {
    let config: IndicatorConfig = confy::load("xcursorlocate").unwrap();

    let padding = 10; // (???) largest circle gets clipped
    let win_size = config.max_size + padding;

    let (conn, screen_num) = xcb::Connection::connect(None)
        .unwrap_or_else(|e| panic!("Unable to connect to X session: {}", e));
    let setup = conn.get_setup();
    let screen = setup
        .roots()
        .nth(screen_num as usize)
        .unwrap_or_else(|| panic!("Error accessing screen!"));

    // fetch all depths
    let depths = screen.allowed_depths();
    let mut alpha_depths = depths.filter(|d| d.depth() == 32u8).peekable();
    if alpha_depths.peek().is_none() {
        panic!("Alpha channel not found!");
    }

    // fetch a visual supporting alpha channels
    let visual = alpha_depths
        .next()
        .unwrap()
        .visuals()
        .nth(1 as usize)
        .unwrap();

    let win_start_x = 0;
    let win_start_y = 0;

    let colormap = conn.generate_id();
    xcb::create_colormap(
        &conn,
        xcb::COLORMAP_ALLOC_NONE as u8,
        colormap,
        screen.root(),
        visual.visual_id(),
    );

    let win = conn.generate_id();
    xcb::create_window(
        &conn,
        32u8,
        win,
        screen.root(),
        win_start_x,
        win_start_y,
        win_size,
        win_size,
        0,
        xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
        visual.visual_id(),
        &[
            (xcb::CW_BACK_PIXEL, 0x00),
            (xcb::CW_BORDER_PIXEL, 0x00), // you need this if you use alpha apparently
            (xcb::CW_OVERRIDE_REDIRECT, 1u32), // dont take focus
            (xcb::CW_EVENT_MASK, xcb::EVENT_MASK_EXPOSURE),
            (xcb::CW_COLORMAP, colormap),
        ],
    );

    let gfx_ctx = conn.generate_id();
    xcb::create_gc(
        &conn,
        gfx_ctx,
        win,
        &[
            (xcb::GC_FOREGROUND, config.color),
            (xcb::GC_GRAPHICS_EXPOSURES, 0),
            (xcb::GC_LINE_WIDTH, config.thickness),
        ],
    );

    xcb::free_colormap(&conn, colormap);

    let win_type_atom = xcb::intern_atom(&conn, true, "_NET_WM_WINDOW_TYPE")
        .get_reply()
        .unwrap()
        .atom();
    let win_menu_atom = xcb::intern_atom(&conn, true, "_NET_WM_WINDOW_TYPE_SPLASH")
        .get_reply()
        .unwrap()
        .atom();

    let win_state_atom = xcb::intern_atom(&conn, true, "_NET_WM_STATE")
        .get_reply()
        .unwrap()
        .atom();
    let win_on_top_atom = xcb::intern_atom(&conn, true, "_NET_WM_STATE_STAYS_ON_TOP")
        .get_reply()
        .unwrap()
        .atom();

    xcb::change_property(
        &conn,
        xcb::PROP_MODE_REPLACE as u8,
        win,
        win_type_atom,
        xcb::ATOM_ATOM,
        32,
        &[win_menu_atom],
    );
    xcb::change_property(
        &conn,
        xcb::PROP_MODE_APPEND as u8,
        win,
        win_state_atom,
        xcb::ATOM_ATOM,
        32,
        &[win_on_top_atom],
    );

    let range: Box<dyn DoubleEndedIterator<Item = u16>> = match config.animation {
        Animation::Grow => Box::new(0..config.no_of_circles),
        Animation::Shrink => Box::new((0..config.no_of_circles).rev()),
    };

    let mut circles = range
        .map(|i| {
            xcb::Arc::new(
                (config.max_size as i16) / (2 * config.no_of_circles as i16) * i as i16, // x
                (config.max_size as i16) / (2 * config.no_of_circles as i16) * i as i16, // y
                (config.max_size) - (config.max_size / config.no_of_circles) * i as u16, // width
                (config.max_size) - (config.max_size / config.no_of_circles) * i as u16, // height
                0,        // start angle
                360 << 6, // end angle
            )
        })
        .collect::<Vec<xcb::Arc>>()
        .into_iter();

    xcb::map_window(&conn, win);
    conn.flush();

    // wait till window is mapped
    let event = conn.wait_for_event();
    match event {
        None => {}
        Some(e) => {
            let r = e.response_type() & !0x80;
            match r {
                // the window is mapped to display
                xcb::EXPOSE => {
                    let pointer = xcb::query_pointer(&conn, win).get_reply().unwrap();
                    let p_x = pointer.root_x();
                    let p_y = pointer.root_y();

                    move_win_to_cursor(&conn, win, win_size, p_x, p_y);

                    let loop_start = Instant::now();
                    let anim_duration = Duration::from_millis(config.duration as u64);
                    let circle_duration =
                        Duration::from_millis((config.duration / config.no_of_circles) as u64);
                    loop {
                        match circles.next() {
                            Some(c) => {
                                let _ = xcb::poly_arc(&conn, win, gfx_ctx, &[c]);
                                conn.flush();
                            }
                            None => {}
                        };
                        thread::sleep(circle_duration);
                        let now = Instant::now();
                        if now.duration_since(loop_start) > anim_duration {
                            break;
                        }
                    }
                    thread::sleep(Duration::from_millis(100));
                }
                _ => {}
            }
        }
    }
}

fn move_win_to_cursor(conn: &xcb::Connection, win: u32, win_size: u16, p_x: i16, p_y: i16) {
    let win_x = p_x - (win_size as i16) / 2;
    let win_y = p_y - (win_size as i16) / 2;
    xcb::configure_window(
        &conn,
        win,
        &[
            (
                xcb::CONFIG_WINDOW_X as u16,
                if win_x < 0 { 0 } else { win_x as u32 },
            ),
            (
                xcb::CONFIG_WINDOW_Y as u16,
                if win_y < 0 { 0 } else { win_y as u32 },
            ),
        ],
    );
    conn.flush();
}
