use dashi::*;
use tare::transient::TransientAllocator;

#[test]
fn transient_allocator_avoids_in_frame_reuse() {
    unsafe {
        std::env::set_var("DASHI_VALIDATION", "0");
    }

    let mut context = Context::headless(&Default::default()).expect("headless context");
    let mut allocator = TransientAllocator::new(&mut context);

    let buffer_info = BufferInfo {
        debug_name: "[TRANSIENT BUFFER]",
        byte_size: 256,
        visibility: MemoryVisibility::Gpu,
        usage: BufferUsage::ALL,
        initial_data: None,
    };

    let first_buffer = allocator.make_buffer(&buffer_info);
    let second_buffer = allocator.make_buffer(&buffer_info);

    assert_ne!(
        first_buffer.handle, second_buffer.handle,
        "buffers allocated in the same frame should not share handles"
    );

    let image_info = ImageInfo {
        debug_name: "[TRANSIENT IMAGE]",
        dim: [4, 4, 1],
        ..Default::default()
    };

    let first_image = allocator.make_image(&image_info);
    let second_image = allocator.make_image(&image_info);

    assert_ne!(
        first_image.view.img, second_image.view.img,
        "images allocated in the same frame should not share handles"
    );
}
