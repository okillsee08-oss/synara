use gpui::{App, Application, Context, IntoElement, Render, Window, WindowOptions, div, px};

struct SynaraView;

impl Render for SynaraView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p(px(32.0))
            .child(div().text_size(px(28.0)).child("Synara"))
            .child(
                div()
                    .pt(px(12.0))
                    .text_size(px(16.0))
                    .child("Rust + GPUI native shell"),
            )
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        cx.open_window(WindowOptions::default(), |_window, cx| {
            cx.new(|_cx| SynaraView)
        })
        .expect("failed to open Synara window");
    });
}
