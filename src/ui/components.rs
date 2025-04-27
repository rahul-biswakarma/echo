use gpui::{Window, div, prelude::*, px, rgb, rgba};

pub struct MainContainer;

impl Render for MainContainer {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size(px(400.0))
            .flex()
            .justify_center()
            .items_center()
            .border_1()
            .border_color(rgba(0xFFFAFA80))
            .shadow_lg()
            .p(px(10.0))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child(format!("Hello"))
    }
}
