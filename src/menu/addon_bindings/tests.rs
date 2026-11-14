use super::*;

fn column(canvas: &Canvas, left: f32, right: f32, top: f32) -> String {
    let draws: Vec<_> = canvas.list.iter().filter(|d| {
        let (qrender::Draw2D::Pic { x, y, .. } | qrender::Draw2D::Char { x, y, .. }
            | qrender::Draw2D::Fill { x, y, .. } | qrender::Draw2D::Tile { x, y, .. }) = d;
        (left..right).contains(x) && (top..top + 16.0).contains(y)
    }).collect();
    format!("{draws:?}")
}

#[test]
fn binding_capture_flashes_only_the_active_device_on_both_screens() {
    for page in [Page::Controls, Page::Addons] {
        for pad in [false, true] {
            let mut menu = Menu::new();
            let mut cv = Cvars::new();
            menu.open(Screen::OptionsPage(page));
            menu.pad_ui = pad;
            menu.bindings[K_MOUSE1 as usize] = Some("+attack".into());
            menu.bindings[K_RTRIGGER as usize] = Some("+attack".into());
            let (columns, top) = if page == Page::Controls {
                ([(VALUE_X, BIND_PAD_X - 4.0), (BIND_PAD_X, PANEL_X + PANEL_W - 2.0)], PAGE_ROW0)
            } else {
                for _ in 0..3 { menu.key(K_TAB, &mut cv, false); }
                ([(425.0, 515.0), (525.0, 615.0)], 282.0)
            };
            let mut canvas = Canvas::new(640, 360);
            let mut ui = crate::font::UiText::default();
            ui.pic = Some(qrender::PicId(0));
            canvas.ui = Some(std::rc::Rc::new(std::cell::RefCell::new(ui)));
            menu.draw(&mut canvas, &mut |_| None, &cv, 0.0);
            let (left, right) = columns[usize::from(!pad)];
            let inactive = column(&canvas, left, right, top);
            assert_ne!(inactive, "[]");
            menu.key(if pad { K_ABUTTON } else { K_ENTER }, &mut cv, false);
            assert!(menu.capturing());
            let mut active = Vec::new();
            for time in [0.0, 0.3] {
                canvas.list.clear();
                menu.draw(&mut canvas, &mut |_| None, &cv, time);
                assert_eq!(column(&canvas, left, right, top), inactive, "{page:?}, pad {pad}");
                let (left, right) = columns[usize::from(pad)];
                active.push(column(&canvas, left, right, top));
            }
            assert_ne!(active[0], active[1], "{page:?}, pad {pad}");
            assert_eq!(active[1], "[]", "{page:?}, pad {pad}");
        }
    }
}
