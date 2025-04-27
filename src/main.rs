use gpui::{App, AppContext, Application};

mod audio;
mod ui;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(ui::window::create_window_options(cx), |_window, cx| {
            cx.new(|_cx| ui::components::MainContainer::default())
        })
        .unwrap();
    })
}
