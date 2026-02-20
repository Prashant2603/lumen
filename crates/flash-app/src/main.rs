mod app;
mod theme;
mod views;
mod widgets;

use app::App;
use iced::window;
use iced::Size;

fn main() -> iced::Result {
    iced::application(App::title, App::update, App::view)
        .theme(App::theme)
        .subscription(App::subscription)
        .window(window::Settings {
            size: Size::new(1200.0, 800.0),
            min_size: Some(Size::new(600.0, 400.0)),
            ..Default::default()
        })
        .run_with(App::new)
}
