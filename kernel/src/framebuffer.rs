use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use core::fmt;
use noto_sans_mono_bitmap::{RasterizedChar, get_raster};
use spin::Mutex;

use crate::{framebuffer::font_constants::BACKUP_CHAR, userspace};

pub static WRITER: Mutex<Option<Writer>> = Mutex::new(None);

const LINE_SPACING: usize = 2;
const LETTER_SPACING: usize = 0;
const BORDER_PADDING: usize = 1;

mod font_constants {
    use noto_sans_mono_bitmap::{FontWeight, RasterHeight, get_raster_width};

    pub const CHAR_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;

    pub const CHAR_RASTER_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_RASTER_HEIGHT);

    pub const BACKUP_CHAR: char = '�';

    pub const FONT_WEIGHT: FontWeight = FontWeight::Regular;
}

fn get_char_raster(character: char) -> RasterizedChar {
    fn get(character: char) -> Option<RasterizedChar> {
        get_raster(
            character,
            font_constants::FONT_WEIGHT,
            font_constants::CHAR_RASTER_HEIGHT,
        )
    }

    get(character).unwrap_or_else(|| get(BACKUP_CHAR).expect("Backup char not found"))
}

pub fn init(framebuffer: FrameBuffer) {
    let mut writer = Writer {
        info: framebuffer.info(),
        buffer: framebuffer,
        x_position: 0,
        y_position: 0,
    };
    writer.clear();

    let mut global_writer = WRITER.try_lock().unwrap();
    assert!(global_writer.is_none(), "Global writer must be None");

    *global_writer = Some(writer);
}

pub struct Writer {
    buffer: FrameBuffer,
    info: FrameBufferInfo,
    x_position: usize,
    y_position: usize,
}

impl Writer {
    fn write_string(&mut self, str: &str) {
        for character in str.chars() {
            self.write_char(character);
        }
    }

    fn write_char(&mut self, character: char) {
        match character {
            '\n' => self.new_line(),
            '\r' => self.carriage_return(),
            character => {
                let updated_x_position = self.x_position + font_constants::CHAR_RASTER_WIDTH;

                if updated_x_position >= self.width() {
                    self.new_line();
                }

                let updated_y_position =
                    self.y_position + font_constants::CHAR_RASTER_HEIGHT.val() + BORDER_PADDING;

                while updated_y_position >= self.height() {
                    self.shift_lines_up();
                }

                self.write_rendered_char(get_char_raster(character));
            }
        }
    }

    fn write_rendered_char(&mut self, rendered_char: RasterizedChar) {
        for (y, row) in rendered_char.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                self.write_pixel(self.x_position + x, self.y_position + y, *byte);
            }
        }

        self.x_position += rendered_char.width() + LETTER_SPACING;
    }

    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = y * self.info.stride + x;

        let color = match self.info.pixel_format {
            PixelFormat::Rgb => [intensity, intensity, intensity / 2, 0],
            PixelFormat::Bgr => [intensity / 2, intensity, intensity, 0],
            PixelFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
            other => {
                self.info.pixel_format = PixelFormat::Rgb;
                panic!("pixel format {:?} not supported", other);
            }
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        unsafe {
            core::arch::asm!("mov r8, r9", in("r9") byte_offset);
        }

        self.buffer.buffer_mut()[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);

        // let _ = unsafe { ptr::read_volatile(&self.buffer.buffer_mut()[byte_offset]) };
    }

    fn new_line(&mut self) {
        self.y_position += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.carriage_return();
    }

    fn carriage_return(&mut self) {
        self.x_position = BORDER_PADDING;
    }

    pub fn clear(&mut self) {
        self.x_position = BORDER_PADDING;
        self.y_position = BORDER_PADDING;
        self.buffer.buffer_mut().fill(0);
    }

    fn shift_lines_up(&mut self) {
        let offset = self.info.stride * self.info.bytes_per_pixel * 8;

        self.buffer.buffer_mut().copy_within(offset.., 0);
        self.y_position += 8;
    }

    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::framebuffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let f = || {
        WRITER.lock().as_mut().unwrap().write_fmt(args).unwrap();
    };

    if !userspace::is_user_ring() {
        interrupts::without_interrupts(f);
    } else {
        f();
    }
}

#[test_case]
fn test_println_simple() {
    println!("test_println_simple output");
}

#[test_case]
fn test_println_many() {
    for _ in 0..200 {
        println!("test_println_many output");
    }
}

#[test_case]
fn test_println_output() {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    let s = "tatakae";

    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writeln!(writer, "\n{}", s).expect("writeln failed");
        for (i, c) in s.chars().enumerate() {
            let screen_char = writer.buffer.chars[BUFFER_HEIGHT - 2][i].read();
            assert_eq!(char::from(screen_char.ascii_character), c);
        }
    });
}
