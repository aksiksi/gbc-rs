#![allow(dead_code)]
use std::path::PathBuf;
use std::time::{Instant, Duration};

use gbc::{Gameboy, Result};
use gbc::joypad::{JoypadEvent, JoypadInput};
use gbc::ppu::{GameboyRgba, LCD_WIDTH, LCD_HEIGHT};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::render::{Canvas, Texture, TextureAccess};
use sdl2::pixels::Color;
use sdl2::video::Window;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Cli {
    #[structopt(parse(from_os_str))]
    rom_file: PathBuf,

    #[structopt(long)]
    boot_rom: bool,

    #[structopt(long)]
    trace: bool,

    #[structopt(default_value = "6", long)]
    scale: u32,

    #[structopt(long)]
    headless: bool,
}

fn keycode_to_joypad_input(keycode: Option<Keycode>) -> Option<JoypadInput> {
    match keycode.unwrap() {
        // TODO: Make key mapping configurable
        Keycode::X => Some(JoypadInput::A),
        Keycode::Z => Some(JoypadInput::B),
        Keycode::A => Some(JoypadInput::Select),
        Keycode::S => Some(JoypadInput::Start),
        Keycode::Up => Some(JoypadInput::Up),
        Keycode::Down => Some(JoypadInput::Down),
        Keycode::Left => Some(JoypadInput::Left),
        Keycode::Right => Some(JoypadInput::Right),
        _ => None,
    }
}

fn event_to_joypad(event: Event) -> Option<JoypadEvent> {
    match event {
        Event::KeyDown { keycode, .. } => {
            if let Some(event) = keycode_to_joypad_input(keycode) {
                Some(JoypadEvent::Down(event))
            } else {
                None
            }
        }
        Event::KeyUp { keycode, .. } => {
            if let Some(event) = keycode_to_joypad_input(keycode) {
                Some(JoypadEvent::Up(event))
            } else {
                None
            }
        }
        _ => unreachable!(),
    }
}

fn render_frame(gameboy: &mut Gameboy, canvas: &mut Canvas<Window>, texture: &mut Texture,
                joypad_events: &[JoypadEvent], outline: bool) {
    // Run the Gameboy for a single frame and return the frame data
    let frame_buffer = gameboy.frame(Some(&joypad_events));

    // With the following, we are setting the texture as a render target for
    // our main canvas. This allows us to use regular canvas drawing functions -
    // e.g., rect, point - to update the underlyinh texture. Note that the texture
    // will be updated only when all canvas operations are complete.
    //
    // Note that, if GPU rendering is enabled, the texture lives in GPU VRAM. If
    // this is the case, updates are fairly expensive, as we need to round-trip
    // to GPU VRAM on every frame (?).
    //
    // Once this closure ends, the canvas target is reset back for us.
    //
    // Helpful C example: https://wiki.libsdl.org/SDL_CreateTexture
    canvas.with_texture_canvas(texture, |canvas| {
        canvas.clear();
        canvas.set_draw_color(Color::BLACK);

        // Draw the rendered frame
        for x in 0..LCD_WIDTH {
            for y in 0..LCD_HEIGHT {
                let GameboyRgba { red, green, blue, alpha } = frame_buffer.read(x, y);
                canvas.set_draw_color(Color::RGBA(red, green, blue, alpha));
                canvas.draw_point((x as i32, y as i32)).unwrap();
            }
        }

        if outline {
            // Draw an outline showing the tiles in the frame
            canvas.set_draw_color(Color::GRAY);

            for row in (0i32..LCD_HEIGHT as i32).step_by(8) {
                canvas.draw_line((0, row), (LCD_WIDTH as i32 - 1, row)).unwrap();
            }

            for col in (0i32..LCD_WIDTH as i32).step_by(8) {
                canvas.draw_line((col, 0), (col, LCD_HEIGHT as i32 - 1)).unwrap();
            }
        }
    }).unwrap();
}

fn gui(cli: Cli) {
    let rom_name = match cli.rom_file.file_name() {
        None => None,
        Some(n) => Some(n.to_str().unwrap()),
    }.unwrap_or("Unknown ROM");

    let title = format!("{} - gbc", rom_name);

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let width = LCD_WIDTH as u32 * cli.scale;
    let height = LCD_HEIGHT as u32 * cli.scale;

    // Setup an SDL2 Window
    let window = video_subsystem.window(&title, width, height)
        .position_centered()
        .allow_highdpi()
        .resizable()
        .build()
        .unwrap();

    // Convert the Window into a Canvas
    // This is what we will use to render content in the Window
    // TODO: Add flag for software vs. GPU
    let mut canvas = window.into_canvas()
                           .software()
                           .build()
                           .unwrap();

    // Fix aspect ratio of canvas
    canvas.set_logical_size(width, height).unwrap();

    // Get a handle to the Canvas texture creator
    let texture_creator = canvas.texture_creator();

    // Create a Texture
    // We write raw pixel data here and copy it to the Canvas for rendering
    let mut texture = texture_creator.create_texture(None,
                                                     TextureAccess::Target,
                                                     LCD_WIDTH as u32,
                                                     LCD_HEIGHT as u32).unwrap();

    let mut gameboy = Gameboy::init(cli.rom_file, cli.boot_rom, cli.trace).unwrap();
    let frame_duration = Duration::new(0, Gameboy::FRAME_DURATION);

    let mut paused = false;
    let mut outline = false;

    // List of joypad events to push to the Gameboy
    let mut joypad_events = Vec::new();

    // Start the event loop
    let mut event_pump = sdl_context.event_pump().unwrap();
    'running: loop {
        let frame_start = Instant::now();

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), .. } => {
                    break 'running
                },
                Event::KeyDown { keycode: Some(Keycode::R), .. } => {
                    // Reset the emulator
                    gameboy.reset();
                }
                Event::KeyDown { keycode: Some(Keycode::P), .. } => {
                    paused = !paused;
                }
                Event::KeyDown { keycode: Some(Keycode::O), .. } => {
                    outline = !outline;
                }
                Event::KeyDown { .. } | Event::KeyUp { .. } => {
                    if let Some(e) = event_to_joypad(event) {
                        joypad_events.push(e);
                    }
                }
                _ => (),
            }
        }

        if !paused {
            // Render a single frame
            render_frame(&mut gameboy, &mut canvas, &mut texture, &joypad_events, outline);

            // Clear out all processed input events
            joypad_events.clear();
        }

        // Once we've completed our texture operations, we need to copy the texture
        // back to the canvas to be able to present it.
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();

        let elapsed = frame_start.elapsed();

        log::debug!("Frame duration: {:?}", elapsed);

        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}

fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::from_args();

    if !cli.headless {
        gui(cli);
    } else {
        let mut gameboy = Gameboy::init(cli.rom_file, false, false)?;
        loop {
            // TODO: Perhaps allow user to provide joypad input file?
            // e.g., list of (input, time)
            gameboy.frame(None);
            std::thread::sleep(Duration::from_nanos(Gameboy::FRAME_DURATION as u64))
        }
    }

    Ok(())
}
