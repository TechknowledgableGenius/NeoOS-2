use std::num::NonZeroU32;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::thread;

use winit::event::{ElementState, Event, MouseButton, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;
use softbuffer::{Context, Surface};

pub type HWND = u32;
pub type ULONG = u32;

#[derive(Debug, Clone)]
pub struct QMSG {
    pub hwnd: HWND,
    pub msg: ULONG,
    pub mp1: u64,
    pub mp2: u64,
    pub time: ULONG,
}

pub const WM_MOUSEMOVE: ULONG = 0x0020;
pub const WM_BUTTON1DOWN: ULONG = 0x0071;
pub const WM_CHAR: ULONG = 0x007A;
pub const WM_QUIT: ULONG = 0x0053;
pub const WM_PAINT: ULONG = 0x0023;

pub struct HMQ {
    pub receiver: Receiver<QMSG>, 
}

#[no_mangle]
pub extern "C" fn WinGetMsg(hmq: &HMQ) -> QMSG {
    match hmq.receiver.recv() {
        Ok(msg) => msg,
        Err(_) => QMSG { hwnd: 0, msg: WM_QUIT, mp1: 0, mp2: 0, time: 0 },
    }
}

pub fn spawn_linux_window_thread(hwnd: HWND, sender: Sender<QMSG>) {
    thread::spawn(move || {
        let event_loop = EventLoop::new().unwrap();
        
        // Wrap the Window in an Arc so it can be shared with softbuffer
        let window = Arc::new(WindowBuilder::new()
            .with_title("Neo OS/2 Subsystem (Linux)")
            .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
            .build(&event_loop)
            .unwrap());

        // softbuffer 0.4 natively accepts Arc<Window> without unsafe blocks
        let context = Context::new(window.clone()).unwrap();
        let mut surface = Surface::new(&context, window.clone()).unwrap();

        let _ = event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CursorMoved { position, .. } => {
                        let msg = QMSG { hwnd, msg: WM_MOUSEMOVE, mp1: ((position.x as u64) << 32) | (position.y as u64), mp2: 0, time: 0 };
                        let _ = sender.send(msg);
                    }
                    WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                        let msg = QMSG { hwnd, msg: WM_BUTTON1DOWN, mp1: 0, mp2: 0, time: 0 };
                        let _ = sender.send(msg);
                    }
                    WindowEvent::KeyboardInput { .. } => {
                        let msg = QMSG { hwnd, msg: WM_CHAR, mp1: 0, mp2: 0, time: 0 }; 
                        let _ = sender.send(msg);
                    }
                    WindowEvent::CloseRequested => {
                        let msg = QMSG { hwnd, msg: WM_QUIT, mp1: 0, mp2: 0, time: 0 };
                        let _ = sender.send(msg);
                        elwt.exit();
                    }
                    WindowEvent::RedrawRequested => {
                        let size = window.inner_size();
                        if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                            surface.resize(w, h).unwrap();
                            // Removed the unnecessary 'mut' keyword from the buffer assignment
                            let mut buffer = surface.buffer_mut().unwrap();
                            buffer.present().unwrap();
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        });
    });
}

pub fn boot_legacy_executable(legacy_path: &str) {
    println!("Initializing x86 CPU Emulator for: {}", legacy_path);
}
