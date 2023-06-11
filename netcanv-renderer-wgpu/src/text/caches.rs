use swash::scale::ScaleContext;
use swash::shape::ShapeContext;

pub struct Caches {
   pub shape_context: ShapeContext,
   pub scale_context: ScaleContext,
}

impl Caches {
   pub fn new() -> Self {
      Self {
         shape_context: ShapeContext::new(),
         scale_context: ScaleContext::new(),
      }
   }
}
