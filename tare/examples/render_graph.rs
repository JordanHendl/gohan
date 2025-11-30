use dashi::*;
use driver::command::DrawIndexed;
use tare::graph::*;


fn main() {
    let mut context = Context::new(&Default::default()).unwrap();
    let mut graph = RenderGraph::new(&mut context);

    let vertex = graph.make_buffer(&BufferInfo::default());
    let indices = graph.make_buffer(&BufferInfo::default());

    let target = graph.make_image(&ImageInfo {
        debug_name: "[ATTACHMENT]",
        dim: [1024, 1024, 1],
        ..Default::default()
    });

    //let subpass_info = {...};
    //let begin_info = {target, ...};
    let stream = CommandStream::new();
    let p_layout = context.make_graphics_pipeline_layout(&GraphicsPipelineLayoutInfo {
        debug_name: todo!(),
        vertex_info: todo!(),
        bg_layouts: todo!(),
        bt_layouts: todo!(),
        shaders: todo!(),
        details: todo!(),
    }).expect("Make Pipeline Layout"); 
    
    let pso = context.make_graphics_pipeline(&GraphicsPipelineInfo::default()).expect("Make graphics pipeline");
    loop {
      graph.add_subpass(&SubpassInfo {
        viewport: todo!(),
        color_attachments: todo!(),
        depth_attachment: todo!(),
        clear_values: todo!(),
        depth_clear: todo!(),
    }, |stream| {          
          let mut s = stream.bind_graphics_pipeline(pso);

          s.draw_indexed(&DrawIndexed{ 
              vertices: vertex.handle,
              indices: indices.handle,
              ..Default::default()
          });
      });
  
      graph.execute();
    }
}

