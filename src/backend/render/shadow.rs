use smithay::backend::renderer::gles::{
    GlesPixelProgram, GlesRenderer, element::PixelShaderElement,
};

use crate::{backend::render::element::AsGlowRenderer, shell::element::CosmicMappedKey};

pub static SHADOW_SHADER: &str = include_str!("./shader/shadow.frag");
pub struct ShadowShader(pub GlesPixelProgram);

pub struct ShadowParameters {
    geo: Rectangle<i32, Local>,
    radius: [u8; 4],
}
type ShadowCache = RefCell<HashMap<CosmicMappedKey, (ShadowParameters, [PixelShaderElement; 4])>>;

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
            let spread = 5.;
            let offset = [0., 5.];
            let color = [0., 0., 0., 0.45];

            let width = softness;
            let sigma = width / 2.;
            let width = ceil(sigma * 3.);

            let offset = Point::new(ceil(offset[0]), ceil(offset[1]));
            let spread = ceil(spread.abs()).copysign(spread);
            let offset = offset - Point::new(spread, spread);
        }
    }
}
