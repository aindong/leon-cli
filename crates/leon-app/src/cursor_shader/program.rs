//! A translated cursor shader compiled with Metal at runtime (no Metal Toolchain needed), cached
//! by its source, and how a frame of it is encoded.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use metal::{
    CompileOptions, Device, MTLClearColor, MTLLoadAction, MTLPixelFormat, MTLPrimitiveType, MTLResourceOptions,
    MTLStoreAction, MTLTextureUsage, RenderPassDescriptor, RenderPipelineDescriptor, RenderPipelineState, Texture,
    TextureDescriptor,
};

use super::{fragment_name, Uniforms};

/// A compiled shader: the pipeline that draws it into a BGRA8 target, and its `iChannel0` (a
/// transparent pixel: the overlay's matte never reads it; a test binds a picture).
pub struct Program {
    pub device: Device,
    pub pipeline: RenderPipelineState,
    pub channel0: Texture,
}

/// Programs alive now, by a hash of their MSL: the same file content compiles once.
fn cache() -> &'static Mutex<HashMap<u64, Weak<Program>>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Weak<Program>>>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}

/// `msl` (from `translate`) compiled on the system's default Metal device, or the one already
/// compiled from the same source.
pub fn compile(msl: &str) -> Result<Arc<Program>, String> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    msl.hash(&mut h);
    let key = h.finish();
    let mut cache = cache().lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = cache.get(&key).and_then(Weak::upgrade) {
        return Ok(p);
    }
    let device = Device::system_default().ok_or("this Mac has no Metal device")?;
    let p = Arc::new(Program::new(&device, msl)?);
    cache.retain(|_, w| w.strong_count() > 0);
    cache.insert(key, Arc::downgrade(&p));
    Ok(p)
}

/// Whether `msl` compiles as the app compiles it; None when this Mac has no Metal device.
pub fn compiles(msl: &str) -> Option<Result<(), String>> {
    Device::system_default()?;
    Some(compile(msl).map(drop))
}

impl Program {
    /// Compile `msl` on `device`.
    pub fn new(device: &Device, msl: &str) -> Result<Self, String> {
        let library = device.new_library_with_source(msl, &CompileOptions::new()).map_err(|e| format!("Metal: {e}"))?;
        let vertex = library.get_function("leon_vertex", None)?;
        let fragment = library.get_function(fragment_name(msl), None)?;
        let desc = RenderPipelineDescriptor::new();
        desc.set_vertex_function(Some(&vertex));
        desc.set_fragment_function(Some(&fragment));
        // no blending: the shader's colour is the layer's pixel, premultiplied as Core Animation reads it
        desc.color_attachments().object_at(0).ok_or("no attachment")?.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
        let pipeline = device.new_render_pipeline_state(&desc)?;
        Ok(Program { device: device.clone(), pipeline, channel0: texture(device, 1, 1, &[0, 0, 0, 0]) })
    }

    /// Encode one frame with `u` into `target`, cleared to transparent first; `channel0` is bound
    /// as `iChannel0` (the program's own when None).
    pub fn encode(
        &self,
        cmd: &metal::CommandBufferRef,
        target: &metal::TextureRef,
        u: &Uniforms,
        channel0: Option<&metal::TextureRef>,
    ) {
        let pass = RenderPassDescriptor::new();
        let a = pass.color_attachments().object_at(0).expect("attachment 0");
        a.set_texture(Some(target));
        a.set_load_action(MTLLoadAction::Clear);
        a.set_clear_color(MTLClearColor::new(0., 0., 0., 0.));
        a.set_store_action(MTLStoreAction::Store);
        let bytes = u.bytes();
        let buf = self.device.new_buffer_with_data(
            bytes.as_ptr().cast(),
            bytes.len() as u64,
            MTLResourceOptions::StorageModeShared,
        );
        let enc = cmd.new_render_command_encoder(pass);
        enc.set_render_pipeline_state(&self.pipeline);
        enc.set_fragment_buffer(0, Some(&buf), 0);
        enc.set_fragment_texture(0, Some(channel0.unwrap_or(&self.channel0)));
        enc.draw_primitives(MTLPrimitiveType::Triangle, 0, 3);
        enc.end_encoding();
    }

    /// Draw one `w`×`h` frame with `u` into a texture of its own, `picture` (BGRA8, row-major)
    /// as `iChannel0`, and read it back as BGRA8 bytes: the overlay's draw, off screen (tests).
    pub fn render(&self, (w, h): (u64, u64), u: &Uniforms, picture: Option<&[u8]>) -> Vec<u8> {
        let target = texture(&self.device, w, h, &vec![0; (w * h * 4) as usize]);
        let channel0 = picture.map(|p| texture(&self.device, w, h, p));
        let queue = self.device.new_command_queue();
        let cmd = queue.new_command_buffer();
        self.encode(cmd, &target, u, channel0.as_deref());
        cmd.commit();
        cmd.wait_until_completed();
        let mut out = vec![0u8; (w * h * 4) as usize];
        target.get_bytes(out.as_mut_ptr().cast(), w * 4, metal::MTLRegion::new_2d(0, 0, w, h), 0);
        out
    }
}

/// Encode a clear to transparent into `target`.
pub fn clear(cmd: &metal::CommandBufferRef, target: &metal::TextureRef) {
    let pass = RenderPassDescriptor::new();
    let a = pass.color_attachments().object_at(0).expect("attachment 0");
    a.set_texture(Some(target));
    a.set_load_action(MTLLoadAction::Clear);
    a.set_clear_color(MTLClearColor::new(0., 0., 0., 0.));
    a.set_store_action(MTLStoreAction::Store);
    cmd.new_render_command_encoder(pass).end_encoding();
}

/// A `w`×`h` BGRA8 texture holding `bgra` (row-major, 4 bytes a pixel), readable on the CPU.
pub fn texture(device: &Device, w: u64, h: u64, bgra: &[u8]) -> Texture {
    let d = TextureDescriptor::new();
    d.set_pixel_format(MTLPixelFormat::BGRA8Unorm);
    d.set_width(w);
    d.set_height(h);
    d.set_storage_mode(metal::MTLStorageMode::Shared);
    d.set_usage(MTLTextureUsage::ShaderRead | MTLTextureUsage::RenderTarget);
    let t = device.new_texture(&d);
    t.replace_region(metal::MTLRegion::new_2d(0, 0, w, h), 0, bgra.as_ptr().cast(), w * 4);
    t
}
