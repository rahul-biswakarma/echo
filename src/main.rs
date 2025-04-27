use gpui::{App, AppContext, Application};

mod ui;

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(ui::window::create_window_options(cx), |_, cx| {
            cx.new(|_cx| ui::components::MainContainer)
        })
        .unwrap();
    })
}
