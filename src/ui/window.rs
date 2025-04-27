use gpui::{
    App, Bounds, SharedString, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowKind, WindowOptions, px, size,
};

pub fn create_window_options(cx: &App) -> WindowOptions {
    let bound = Bounds::centered(None, size(px(400.), px(300.)), cx);
    let title_bar = TitlebarOptions {
        title: Some(SharedString::new_static("Echo")),
        appears_transparent: true,
        traffic_light_position: None,
    };

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bound)),
        titlebar: Some(title_bar),
        app_id: Some(String::from("echo")),
        display_id: None,
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        window_min_size: Some(size(px(400.0), px(300.0))),
        is_movable: true,
        window_background: WindowBackgroundAppearance::Blurred,
        window_decorations: Some(WindowDecorations::Server),
    }
}
