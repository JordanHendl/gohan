use dashi::*;
use sdl2::{event::Event, keyboard::Keycode};
use std::time::{Duration, Instant};
pub struct Timer {
    start_time: Option<Instant>,
    elapsed: Duration,
    is_paused: bool,
}

impl Timer {
    // Create a new timer instance
    pub fn new() -> Timer {
        Timer {
            start_time: None,
            elapsed: Duration::new(0, 0),
            is_paused: false,
        }
    }

    // Start the timer
    pub fn start(&mut self) {
        if self.start_time.is_none() {
            self.start_time = Some(Instant::now());
        } else if self.is_paused {
            // Resume from where it was paused
            self.start_time = Some(Instant::now() - self.elapsed);
            self.is_paused = false;
        }
    }

    // Stop the timer
    pub fn stop(&mut self) {
        if let Some(start_time) = self.start_time {
            self.elapsed = start_time.elapsed();
            self.start_time = None;
            self.is_paused = false;
        }
    }

    // Pause the timer
    pub fn pause(&mut self) {
        if let Some(start_time) = self.start_time {
            self.elapsed = start_time.elapsed();
            self.is_paused = true;
            self.start_time = None;
        }
    }

    // Reset the timer
    pub fn reset(&mut self) {
        self.start_time = None;
        self.elapsed = Duration::new(0, 0);
        self.is_paused = false;
    }

    // Get the current elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u128 {
        if let Some(start_time) = self.start_time {
            if self.is_paused {
                self.elapsed.as_millis()
            } else {
                start_time.elapsed().as_millis()
            }
        } else {
            self.elapsed.as_millis()
        }
    }
}

pub struct ImageLoadInfo<T> {
    pub filename: String,
    pub size: [u32; 2],
    pub format: dashi::Format,
    pub bytes: Vec<T>,
}

impl<T> ImageLoadInfo<T> {
    pub fn into_gpu(&self, ctx: &mut Context) -> miso::TextureInfo {
        let img = ctx
            .make_image(&ImageInfo {
                debug_name: &self.filename,
                dim: [self.size[0], self.size[1], 1],
                layers: 1,
                format: self.format,
                mip_levels: 1,
                initial_data: unsafe { Some(self.bytes.align_to::<u8>().1) },
            })
            .unwrap();

        let view = ctx
            .make_image_view(&ImageViewInfo {
                debug_name: &self.filename,
                img,
                ..Default::default()
            })
            .unwrap();

        let sampler = ctx
            .make_sampler(&SamplerInfo {
                ..Default::default()
            })
            .unwrap();

        miso::TextureInfo {
            image: img,
            view,
            sampler,
            dim: self.size,
        }
    }
}

pub fn load_image_rgba8(path: &str) -> ImageLoadInfo<u8> {
    println!("Loading {}", path);
    let img = image::open(&path).unwrap();

    // Convert the image to RGBA8 format
    let rgba_image = img.to_rgba8();

    // Flip the image vertically (upside down)
    //    let rgba_image = image::imageops::flip_vertical(&rgba_image);

    let (width, height) = rgba_image.dimensions();
    let bytes = rgba_image.into_raw();
    assert!((width * height * 4) as usize == bytes.len());
    ImageLoadInfo::<u8> {
        filename: path.to_string(),
        size: [width, height],
        format: dashi::Format::RGBA8,
        bytes,
    }
}

#[cfg(feature = "miso-tests")]
fn main() {
    use glam::{vec2, vec3, vec4, Mat4};
    use miso::{DirectionalLightInfo, MaterialInfo, MeshInfo, ObjectInfo, Vertex};

    let img = format!("{}/test/assets/default.png", env!("CARGO_MANIFEST_DIR"));
    let device = DeviceSelector::new()
        .unwrap()
        .select(DeviceFilter::default().add_required_type(DeviceType::Dedicated))
        .unwrap_or_default();

    println!("Using device {}", device);

    let mut ctx = dashi::Context::new(&ContextInfo {
        device,
        ..Default::default()
    })
    .unwrap();
    let mut scene = miso::Scene::new(&mut ctx, &miso::SceneInfo { cfg: None });
    let mut event_pump = ctx.get_sdl_ctx().event_pump().unwrap();
    let mut timer = Timer::new();

    timer.start();

    let vert_buffer: [Vertex; 3] = [
        Vertex {
            position: vec4(0.0, -0.5, 0.0, 1.0),
            normal: vec4(0.2, -0.5, 0.0, 1.0),
            tex_coords: vec2(0.0, -0.5),
            ..Default::default()
        }, // Vertex 0: Bottom
        Vertex {
            position: vec4(0.5, 0.5, 0.0, 1.0),
            normal: vec4(0.5, 0.5, 0.0, 1.0),
            tex_coords: vec2(0.5, 0.5),
            ..Default::default()
        }, // Vertex 0: Bottom
        Vertex {
            position: vec4(-0.5, 0.5, 0.0, 1.0),
            normal: vec4(-0.5, 0.5, 0.0, 1.0),
            tex_coords: vec2(-0.5, 0.5),
            ..Default::default()
        }, // Vertex 0: Bottom
    ];

    const INDICES: [u32; 3] = [
        0, 1, 2, // Triangle: uses vertices 0, 1, and 2
    ];

    // Allocate the vertices & indices.
    let vertices = ctx
        .make_buffer(&BufferInfo {
            debug_name: "vertices",
            byte_size: (vert_buffer.len() * std::mem::size_of::<Vertex>()) as u32,
            visibility: MemoryVisibility::Gpu,
            usage: BufferUsage::VERTEX,
            initial_data: unsafe { Some(vert_buffer.align_to::<u8>().1) },
        })
        .unwrap();

    let indices = ctx
        .make_buffer(&BufferInfo {
            debug_name: "indices",
            byte_size: (INDICES.len() * std::mem::size_of::<u32>()) as u32,
            visibility: MemoryVisibility::Gpu,
            usage: BufferUsage::INDEX,
            initial_data: unsafe { Some(INDICES.align_to::<u8>().1) },
        })
        .unwrap();

    let mesh = scene.register_mesh(&MeshInfo {
        name: "hello-triangle triangle".to_string(),
        vertices,
        num_vertices: vert_buffer.len(),
        indices,
        num_indices: INDICES.len(),
    });

    let base_color = scene.register_texture(&load_image_rgba8(&img).into_gpu(&mut ctx));

    let material = scene.register_material(&MaterialInfo {
        name: "hello-triangle".to_string(),
        passes: vec!["non-transparent".to_string()],
        base_color,
        ..Default::default()
    });

    let object = scene.register_object(&ObjectInfo {
        mesh,
        material,
        transform: Default::default(),
    });
    
    let _dirlight = scene.register_directional_light(&DirectionalLightInfo {
        direction: vec4(-0.7, -1.0, -0.7, 1.0),
        color: glam::vec4(0.8, 0.8, 0.8, 1.0),
        intensity: 0.7,
    });

    'running: loop {
        // Listen to events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    break 'running;
                }
                _ => {}
            }
        }

        let pos = vec3(
            (timer.elapsed_ms() as f32 / 1000.0).sin() * 0.5,
            (timer.elapsed_ms() as f32 / 1000.0).cos() * 0.5,
            0.0,
        );

        scene.update_object_transform(object, &Mat4::from_translation(pos));
        scene.update();
    }
    //
    ctx.clean_up();
}

#[cfg(not(feature = "miso-tests"))]
fn main() { //none
}
