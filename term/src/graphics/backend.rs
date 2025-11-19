use mousefood::{EmbeddedBackend, EmbeddedBackendConfig, prelude::Rgb888};
use ratatui::Terminal;
use tinygraphics::{
    backend::PrimitiveDrawer,
    prelude::{DrawTarget, RgbColor},
};

pub type Color = Rgb888;
#[allow(type_alias_bounds)]
pub type DrawBackend<'a, C: RgbColor> =
    PrimitiveDrawer<'a, tinygraphics::backend::KernelFBWrapper, C>;
#[allow(type_alias_bounds)]
pub type MousefoodBackend<'a, D: DrawTarget> = EmbeddedBackend<'a, D, D::Color>;

pub fn init_drawer<C: RgbColor>() -> DrawBackend<'static, C> {
    DrawBackend::default()
}

pub fn init_backend<'draw>(
    drawer: &'draw mut DrawBackend<'static, Color>,
) -> MousefoodBackend<'draw, DrawBackend<'static, Color>> {
    MousefoodBackend::new(drawer, EmbeddedBackendConfig::default())
}

pub fn init_term(
    backend: MousefoodBackend<'static, DrawBackend<'static, Color>>,
) -> Result<Terminal<MousefoodBackend<'static, DrawBackend<'static, Color>>>, mousefood::error::Error>
{
    Terminal::new(backend)
}
