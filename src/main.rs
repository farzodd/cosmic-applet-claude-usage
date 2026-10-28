mod api;
mod app;
mod chart;
mod config;

fn main() -> cosmic::iced::Result {
    cosmic::applet::run::<app::Applet>(())
}
