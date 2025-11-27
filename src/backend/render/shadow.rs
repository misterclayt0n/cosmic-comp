use std::{borrow::Borrow, cell::RefCell, collections::HashMap};

use smithay::{
    backend::renderer::{
        element::Kind,
        gles::{GlesPixelProgram, GlesRenderer, Uniform, element::PixelShaderElement},
    },
    utils::{IsAlive, Point, Rectangle, Size},
};

use crate::{
    backend::render::element::AsGlowRenderer, shell::element::CosmicMappedKey,
    utils::prelude::Local,
};

pub static SHADOW_SHADER: &str = include_str!("./shaders/shadow.frag");
pub struct ShadowShader(pub GlesPixelProgram);

#[derive(Debug, PartialEq)]
pub struct ShadowParameters {
    geo: Rectangle<i32, Local>,
    radius: [u8; 4],
}
type ShadowCache = RefCell<HashMap<CosmicMappedKey, (ShadowParameters, Vec<PixelShaderElement>)>>;

impl ShadowShader {
    pub fn get<R: AsGlowRenderer>(renderer: &R) -> GlesPixelProgram {
        Borrow::<GlesRenderer>::borrow(renderer.glow_renderer())
            .egl_context()
            .user_data()
            .get::<ShadowShader>()
            .expect("Custom Shaders not initialized")
            .0
            .clone()
    }

    pub fn shadow_elements<R: AsGlowRenderer>(
        renderer: &R,
        key: CosmicMappedKey,
        geo: Rectangle<i32, Local>,
        radius: [u8; 4],
        scale: f64,
        alpha: f64,
    ) -> impl Iterator<Item = PixelShaderElement> {
        let params = ShadowParameters { geo, radius };

        let user_data = Borrow::<GlesRenderer>::borrow(renderer.glow_renderer())
            .egl_context()
            .user_data();

        user_data.insert_if_missing(|| ShadowCache::new(HashMap::new()));
        let mut cache = user_data.get::<ShadowCache>().unwrap().borrow_mut();
        cache.retain(|k, _| k.alive());

        if cache
            .get(&key)
            .filter(|(old_params, _)| &params == old_params)
            .is_none()
        {
            let shader = Self::get(renderer);
            let ceil = |logical: f64| (logical * scale).ceil() / scale;

            let softness = 30.;
            let spread: f64 = 5.;
            let offset = [0., 5.];
            let color = [0., 0., 0., 0.45];

            let width = softness;
            let sigma = width / 2.;
            let width = ceil(sigma * 3.);

            let offset = Point::new(ceil(offset[0]), ceil(offset[1]));
            let spread = ceil(spread.abs()).copysign(spread);
            let offset = offset - Point::new(spread, spread);

            let box_size = if spread >= 0. {
                geo + Size::new(spread, spread).upscale(2.)
            } else {
                geo - Size::new(-spread, -spread).upscale(2.)
            };

            let win_radius = radius;
            let radius = radius.map(|r| {
                if r > 0 {
                    r.saturating_add_signed(spread.round() as i8)
                } else {
                    0
                }
            });
            let shader_size = box_size + Size::from((width, width)).upscale(2.);
            let shader_geo = Rectangle::new(Point::from((-width, -width)), shader_size);

            // This is actually offset relative to shader_geo, this is handled below.
            let window_geo = Rectangle::new(Point::from((0., 0.)), geo);

            let top_left = ceil(f64::from(radius[0]));
            let top_right = f64::min(geo.w - top_left, ceil(f64::from(radius[1])));
            let bottom_left = f64::min(geo.h - top_left, ceil(f64::from(radius[2])));
            let bottom_right = f64::min(
                geo.h - top_right,
                f64::min(geo.w - bottom_left, ceil(f64::from(radius[3]))),
            );

            let top_left = Rectangle::new(Point::from((0., 0.)), Size::from((top_left, top_left)));
            let top_right = Rectangle::new(
                Point::from((geo.w - top_right, 0.)),
                Size::from((top_right, top_right)),
            );
            let bottom_right = Rectangle::new(
                Point::from((geo.w - bottom_right, geo.h - bottom_right)),
                Size::from((bottom_right, bottom_right)),
            );
            let bottom_left = Rectangle::new(
                Point::from((0., geo.h - bottom_left)),
                Size::from((bottom_left, bottom_left)),
            );

            let mut background =
                window_geo.subtract_rects([top_left, top_right, bottom_right, bottom_left]);
            for rect in &mut background {
                rect.loc -= offset;
            }

            let elements = Vec::with_capacity(4);
            for mut rect in shader_geo.subtract_rects(background) {
                let window_geo =
                    Rectangle::new(window_geo.loc - offset - rect.loc, window_geo.size);
                rect.loc += offset;

                elements.push(PixelShaderElement::new(
                    shader,
                    rect,
                    None,
                    alpha,
                    vec![
                        Uniform::new("shadow_color", color),
                        Uniform::new("sigma", sigma),
                        mat3_uniform("input_to_geo", input_to_geo),
                        Uniform::new("geo_size", [box_size.w, box_size.h]),
                        Uniform::new("corner_radius", <[f32; 4]>::from(radius)),
                        mat3_uniform("window_input_to_geo", window_input_to_geo),
                        Uniform::new("window_geo_size", [window_geo.size.w, window_geo.size.h]),
                        Uniform::new("window_corner_radius", <[f32; 4]>::from(win_radius)),
                    ],
                    Kind::Unspecified,
                ))
            }

            cache.insert(key, (params, elements));
        }

        cache.get(&key).unwrap()[1].iter().cloned()
    }
}
