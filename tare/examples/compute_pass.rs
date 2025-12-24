use dashi::driver::command::CopyBuffer;
use dashi::*;
use tare::graph::RenderGraph;

fn main() {
    let mut context = Context::headless(&Default::default()).expect("headless context");
    let mut graph = RenderGraph::new(&mut context);

    let value_bytes = 42u32.to_le_bytes();

    let mut output = graph.make_buffer(&BufferInfo {
        debug_name: "[OUTPUT]",
        byte_size: std::mem::size_of::<u32>() as u32,
        visibility: MemoryVisibility::CpuAndGpu,
        usage: BufferUsage::ALL,
        initial_data: Some(&[0u8; std::mem::size_of::<u32>()]),
    });
    output.size = std::mem::size_of::<u32>() as u64;

    let mut source = graph.make_buffer(&BufferInfo {
        debug_name: "[SOURCE]",
        byte_size: std::mem::size_of::<u32>() as u32,
        visibility: MemoryVisibility::Gpu,
        usage: BufferUsage::ALL,
        initial_data: Some(&value_bytes),
    });
    source.size = std::mem::size_of::<u32>() as u64;

    graph.add_compute_pass(move |stream| {
        stream
            .copy_buffers(&CopyBuffer {
                src: source.handle,
                dst: output.handle,
                src_offset: 0,
                dst_offset: 0,
                amount: std::mem::size_of::<u32>() as u32,
            })
            .end()
    });

    graph.execute();
    context.sync_current_device();

    let data = context
        .map_buffer::<u32>(output.handle.into())
        .expect("map compute output buffer")
        .to_vec();
    context
        .unmap_buffer(output.handle)
        .expect("unmap compute output buffer");

    println!("Compute pass wrote: {:?}", data);
}
