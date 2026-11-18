use super::Shape;

/// Consumes shared map geometry without knowing how terrain was generated.
/// Canvas and vector export implement the same interface.
pub trait SceneRenderer {
    fn render_shape(&mut self, shape: &Shape);

    fn render_scene(&mut self, shapes: &[Shape]) {
        for shape in shapes {
            self.render_shape(shape);
        }
    }
}
