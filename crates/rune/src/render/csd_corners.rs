//! Clearing the corners a Wayland client-side frame leaves outside its rounded window.
//!
//! Under `sctk-adwaita` the frame draws the window's shadow and its one-point visible border on
//! subsurfaces around the content, and rounds the window's corners while it floats
//! (`vendor/sctk-adwaita`). The content's own surface is a rectangle, so its square corners would
//! show past that border. This clears the pixels outside the arc at the two bottom corners, which
//! the frame's border covers, so the surface composites them fully transparent. The top corners
//! need nothing: the headerbar's own surface covers the content's top edge.

/// A pipeline that writes transparent pixels outside a rounded corner's arc, and nothing inside
/// it. Its blend is off, so every other pixel of the target keeps what it had: only each corner's
/// square is drawn at all, and within it the arc's outside alone.
pub(crate) struct CsdCorners {
    pipeline: wgpu::RenderPipeline,
    format: wgpu::TextureFormat,
    vertices: wgpu::Buffer,
    /// The radius, in device pixels, and target size `vertices` were written for.
    prepared: Option<(f32, (u32, u32))>,
}

impl CsdCorners {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rounded corners"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
// `square` is the vertex's place in its corner's own square, measured from the arc's centre: the
// centre is (0, 0), where the arc meets the window's edge (-1, 0), and the window's corner
// (-1, 1). Everything with length > 1 is outside the arc.
struct Vertex {
    @builtin(position) at: vec4<f32>,
    @location(0) square: vec2<f32>,
};

@vertex fn vs(@location(0) at: vec2<f32>, @location(1) square: vec2<f32>) -> Vertex {
    return Vertex(vec4(at, 0.0, 1.0), square);
}

@fragment fn fs(frag: Vertex) -> @location(0) vec4<f32> {
    // Inside the arc the content stays as it was drawn; everything outside it is clear, so only
    // the frame's border is left there.
    if length(frag.square) <= 1.0 {
        discard;
    }
    return vec4(0.0);
}
"#
                .into(),
            ),
        });
        let stride = 4 * std::mem::size_of::<f32>() as wgpu::BufferAddress;
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rounded corners"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: stride,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: stride / 2, shader_location: 1 },
                    ],
                })],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rounded corners"),
            // Two corners, two triangles each, four values a vertex.
            size: 12 * stride,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self { pipeline, format, vertices, prepared: None }
    }

    /// The format it draws into: a surface that changed its format needs another pass.
    pub fn format(&self) -> wgpu::TextureFormat { self.format }

    /// Whether it already clears `size` with arcs of `radius` device pixels.
    pub fn is_prepared_for(&self, size: (u32, u32), radius: f32) -> bool { self.prepared == Some((radius, size)) }

    /// Writes the two bottom corner squares of a `size` target, whose arcs have `radius` device
    /// pixels.
    pub fn prepare(&mut self, queue: &wgpu::Queue, size: (u32, u32), radius: f32) {
        if self.is_prepared_for(size, radius) { return; }
        let (width, height) = (size.0.max(1) as f32, size.1.max(1) as f32);
        let mut vertices: Vec<f32> = Vec::with_capacity(12 * 4);
        // Each corner's square runs from the arc's centre out to the window's edges, a radius
        // either way; the bottom two sit a radius in from the foot's sides and off its edge, one
        // reaching left and the other right.
        for (centre, side) in [((radius, height - radius), -1.), ((width - radius, height - radius), 1.)] {
            for (u, v) in [(0., 0.), (1., 0.), (0., 1.), (1., 0.), (1., 1.), (0., 1.)] {
                let (x, y) = (centre.0 + side * u * radius, centre.1 + v * radius);
                vertices.extend_from_slice(&[
                    x / width * 2.0 - 1.0,
                    1.0 - y / height * 2.0,
                    u,
                    v,
                ]);
            }
        }
        queue.write_buffer(&self.vertices, 0, bytemuck::cast_slice(&vertices));
        self.prepared = Some((radius, size));
    }

    /// Clears outside the arcs, leaving every other pixel of `target` as it is.
    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rounded corners"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.draw(0..12, 0..1);
    }
}

#[cfg(test)]
#[path = "csd_corners_tests.rs"]
mod tests;
