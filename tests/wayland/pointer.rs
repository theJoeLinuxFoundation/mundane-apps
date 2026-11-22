//! Send titlebar gestures using one virtual pointer on the test compositor.
use std::{
    env,
    thread::sleep,
    time::{Duration, Instant},
};
use wayland_client::{
    Connection, Dispatch, QueueHandle, delegate_noop,
    globals::{GlobalListContents, registry_queue_init},
    protocol::wl_registry,
};
use wayland_protocols_wlr::virtual_pointer::v1::client::{
    zwlr_virtual_pointer_manager_v1::ZwlrVirtualPointerManagerV1,
    zwlr_virtual_pointer_v1::ZwlrVirtualPointerV1,
};

struct Client;
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Client {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
delegate_noop!(Client: ignore ZwlrVirtualPointerManagerV1);
delegate_noop!(Client: ignore ZwlrVirtualPointerV1);

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let socket = env::args_os()
        .nth(1)
        .ok_or("expected private test socket")?;
    let connection = Connection::from_socket(std::os::unix::net::UnixStream::connect(socket)?)?;
    let (globals, queue) = registry_queue_init::<Client>(&connection)?;
    let handle = queue.handle();
    let manager: ZwlrVirtualPointerManagerV1 = globals.bind(&handle, 1..=2, ())?;
    let pointer = manager.create_virtual_pointer(None, &handle, ());
    let start = Instant::now();
    let time = || start.elapsed().as_millis() as u32;
    let frame = || -> Result<(), Box<dyn std::error::Error>> {
        pointer.frame();
        connection.roundtrip()?;
        sleep(Duration::from_millis(50));
        Ok(())
    };
    if env::args().nth(2).as_deref() == Some("gesture") {
        use wayland_client::protocol::wl_pointer::ButtonState;
        for action in env::args().skip(3) {
            match action.as_str() {
                "wait" => {
                    use std::io::{self, Write};
                    println!("ready");
                    io::stdout().flush()?;
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line)? == 0 {
                        return Ok(());
                    }
                }
                "press" => pointer.button(time(), 0x110, ButtonState::Pressed),
                "release" => pointer.button(time(), 0x110, ButtonState::Released),
                pair => {
                    let (x, y) = pair
                        .split_once(',')
                        .ok_or("expected X,Y on 800 × 500 output")?;
                    pointer.motion_absolute(time(), x.parse()?, y.parse()?, 800, 500);
                }
            }
            frame()?;
        }
        return Ok(());
    }
    pointer.motion_absolute(time(), 100, 20, 640, 360);
    frame()?;
    let drag = env::args().nth(2).as_deref() == Some("drag");
    for _ in 0..if drag { 1 } else { 2 } {
        pointer.button(
            time(),
            0x110,
            wayland_client::protocol::wl_pointer::ButtonState::Pressed,
        );
        frame()?;
        if drag {
            pointer.motion_absolute(time(), 118, 22, 640, 360);
            frame()?;
        }
        pointer.button(
            time(),
            0x110,
            wayland_client::protocol::wl_pointer::ButtonState::Released,
        );
        frame()?;
    }
    Ok(())
}
