use crate::IndicatorConfig;
use std::thread;
use std::time::Duration;

pub struct Animation {
    duration_per_frame: Duration,
    frames: Vec<xcb::Arc>,
    clear: bool,
}

impl Animation {
    pub fn circles(
        config: IndicatorConfig,
        function: Box<dyn DoubleEndedIterator<Item = u16>>,
        clear: bool,
    ) -> Animation {
        let no_of_frames = config.framerate;
        let duration_per_frame =
            Duration::from_millis(config.duration).div_f64(no_of_frames as f64);
        let frames = function
            .map(|i| {
                xcb::Arc::new(
                    ((config.max_size as f32) / (2. * no_of_frames as f32) * i as f32) as i16, // x
                    ((config.max_size as f32) / (2. * no_of_frames as f32) * i as f32) as i16, // y
                    (config.max_size) - (config.max_size / no_of_frames) * i as u16, // width
                    (config.max_size) - (config.max_size / no_of_frames) * i as u16, // height
                    0,                                                               // start angle
                    360 << 6,                                                        // end angle
                )
            })
            .collect::<Vec<xcb::Arc>>();
        return Animation {
            duration_per_frame,
            frames,
            clear,
        };
    }
    pub fn play(&self, conn: &xcb::Connection, win: u32, gfx_ctx: u32) {
        for frame in &self.frames {
            xcb::poly_arc(&conn, win, gfx_ctx, &[*frame]);
            conn.flush();
            if self.clear {
                xcb::clear_area(&conn, true, win, frame.x(), frame.y(), 2000, 2000);
            }
            thread::sleep(self.duration_per_frame);
        }
    }
}
