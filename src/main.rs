use std::thread;
use std::time::{Duration, Instant};

struct Configuration {
    max_size: u16,  // display pixels
    duration: u16,  // milliseconds
    thickness: u32, // display pixels
    no_of_circles: u16,
}

fn main() {
    let config = Configuration {
        max_size: 200u16,
        duration: 600u16,
        thickness: 1,
        no_of_circles: 6,
    };
    let win_width = config.max_size;
    let win_height = config.max_size;

    let (conn, screen_num) = xcb::Connection::connect(None)
        .unwrap_or_else(|e| panic!("Unable to connect to X session: {}", e));
    let setup = conn.get_setup();
    let screen = setup
        .roots()
        .nth(screen_num as usize)
        .unwrap_or_else(|| panic!("Error accessing screen!"));

    let depths = screen.allowed_depths();
    let mut alpha_depths = depths.filter(|d| d.depth() == 32u8).peekable();
    if alpha_depths.peek().is_none() {
        panic!("Alpha channel not found!");
    }

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
        win_width,
        win_height,
        0,
        xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
        visual.visual_id(),
        &[
            (xcb::CW_BACK_PIXEL, 0x00),
            (xcb::CW_BORDER_PIXEL, 0x00),
            //(xcb::CW_OVERRIDE_REDIRECT, 1u32),
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
            (xcb::GC_FOREGROUND, screen.white_pixel()),
            (xcb::GC_GRAPHICS_EXPOSURES, 0),
            (xcb::GC_LINE_WIDTH, 1),
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

    let mut circles = (1u16..10u16)
        .map(|i| {
            xcb::Arc::new(
                win_start_x + 25 * i as i16,
                win_start_y + 25 * i as i16,
                win_width - 50 * i,
                win_height - 50 * i,
                0,
                360 << 6,
            )
        })
        .collect::<Vec<xcb::Arc>>()
        .into_iter();

    xcb::map_window(&conn, win);
    conn.flush();

    loop {
        let event = conn.wait_for_event();
        match event {
            None => {
                break;
            }
            Some(e) => {
                let r = e.response_type() & !0x80;
                match r {
                    xcb::EXPOSE => {
                        xcb::poly_arc(&conn, win, gfx_ctx, &[circles.next().unwrap()]);
                        conn.flush();
                        break;
                    }
                    _ => {
                        break;
                    }
                }
            }
        }
    }

    let pointer = xcb::query_pointer(&conn, win).get_reply().unwrap();
    let p_x = pointer.root_x();
    let p_y = pointer.root_y();

    move_win_to_cursor(&conn, win, win_width, win_height, p_x, p_y);

    let loop_start = Instant::now();
    let anim_duration = Duration::from_millis(600);
    let circle_duration = Duration::from_millis(60);
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

fn move_win_to_cursor(
    conn: &xcb::Connection,
    win: u32,
    win_width: u16,
    win_height: u16,
    p_x: i16,
    p_y: i16,
) {
    let win_x = p_x - (win_width as i16) / 2;
    let win_y = p_y - (win_height as i16) / 2;
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
