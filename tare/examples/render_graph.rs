
fn color_image_attachment() -> ImageInfo {

}

fn main() {
    let mut context = Context::new(&Default::default()).unwrap();

    let vertex = ...;
    let indices = ...;

    let graph = RenderGraph::new(context);
    let target = graph.make_image(color_image_attachment);
    let subpass_info = {...};
    let begin_info = {target, ...};
    let stream = CommandStream::new();

    while true {
      graph.begin_drawing(begin_info, stream);
      graph.begin_subpass(&subpass_info, |stream| {
          let pso = graph.build_pso(&PSOInfo {
            ...
          });
          
          stream.bind_graphics_pipeline(pso.pipeline)
          stream.draw_indexed(&DrawIndexed{ 
              vertices: vertex,
              indices: indices,
              ..Default::default(),
          });
      });
  
      graph.execute();
    }
}

