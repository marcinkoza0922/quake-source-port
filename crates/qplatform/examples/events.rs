//! Opens a window and prints platform events until quit (Esc or close).
fn main() -> Result<(), qplatform::Error> {
    let mut p = qplatform::Platform::init()?;
    let w = p.create_window("qplatform events", 800, 600, false)?;
    w.set_text_input(true)?;
    println!("drawable {:?}", w.drawable_size());
    loop {
        while let Some(e) = p.poll_event() {
            println!("{e:?}");
            match e {
                qplatform::Event::Quit => return Ok(()),
                qplatform::Event::Key { scancode: 41, down: true, .. } => return Ok(()),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
