//! Clearing a window's rounded bottom corners on the GPU: what stays, and what goes.

use super::CsdCorners;
use crate::render::{Gpu, Offscreen, OFFSCREEN_FORMAT};

/// A target of opaque red, its corners cleared at `radius` device pixels.
fn cleared(gpu: &Gpu, size: (u32, u32), radius: f32) -> Vec<u8> {
    let target = Offscreen::new(&gpu.device, size.0, size.1);
    let mut pass = CsdCorners::new(&gpu.device, OFFSCREEN_FORMAT);
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    {
        let _clear = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }
    pass.prepare(&gpu.queue, (size.0, size.1), radius);
    pass.draw(&mut enc, &target.view);
    target.copy(&mut enc);
    gpu.queue.submit([enc.finish()]);
    target.read(&gpu.device)
}

/// The target's pixel at `(x, y)`, as RGBA.
fn at(pixels: &[u8], width: u32, (x, y): (u32, u32)) -> [u8; 4] {
    let start = ((y * width + x) * 4) as usize;
    pixels[start..start + 4].try_into().expect("four channels")
}

#[test]
fn the_bottom_corners_clear_and_nothing_else() {
    let gpu = Gpu::new(Gpu::instance(), None);
    let (width, height) = (40, 30);
    let radius = 6.;
    let pixels = cleared(&gpu, (width, height), radius);
    let red = at(&pixels, width, (0, 0));
    assert_eq!(red[3], 255, "the top left corner keeps its pixel: {red:?}");
    assert_eq!(at(&pixels, width, (0, height - 1))[3], 0, "the bottom left corner clears");
    assert_eq!(at(&pixels, width, (width - 1, height - 1))[3], 0, "the bottom right corner clears");
    // Along the foot, the arc is gone only past where it meets the window's edge.
    assert_eq!(at(&pixels, width, (radius as u32, height - 1))[3], 255, "inside the arc stays");
    assert_eq!(at(&pixels, width, (width - 1 - radius as u32, height - 1))[3], 255, "inside the arc stays");
    // Above the corners' own squares nothing changed, at either side.
    assert_eq!(at(&pixels, width, (0, height - 1 - radius as u32))[3], 255, "above the arc's centre stays");
    assert_eq!(at(&pixels, width, (width - 1, height - 1 - radius as u32))[3], 255, "above the arc's centre stays");
    // The top two corners are the headerbar's business.
    assert_eq!(at(&pixels, width, (width - 1, 0))[3], 255, "the top right corner keeps its pixel");
}

/// The arcs follow the radius: a wider one takes more of the corner.
#[test]
fn a_wider_radius_clears_more_of_the_corner() {
    let gpu = Gpu::new(Gpu::instance(), None);
    let size = (40, 30);
    let narrow = cleared(&gpu, size, 4.);
    let wide = cleared(&gpu, size, 9.);
    // A pixel 5 in from the bottom left: inside a radius-9 arc's corner square but outside its
    // arc, and outside a radius-4 one's square altogether.
    let corner = (5, 29);
    assert_eq!(at(&narrow, size.0, corner)[3], 255, "outside the narrow corner's square");
    assert_eq!(at(&wide, size.0, corner)[3], 0, "outside the wide corner's arc");
}

/// A square window (maximized or tiled) draws nothing at all: no radius, no corner.
#[test]
fn a_square_window_keeps_every_pixel() {
    let gpu = Gpu::new(Gpu::instance(), None);
    let size = (40, 30);
    let pixels = cleared(&gpu, size, 0.);
    for y in 0..size.1 {
        for x in 0..size.0 {
            assert_eq!(at(&pixels, size.0, (x, y))[3], 255, "({x}, {y}) cleared with no radius");
        }
    }
}
