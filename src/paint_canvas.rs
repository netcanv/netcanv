use std::cell::RefCell;
use std::rc::Rc;

use skulpin::skia_safe::*;

pub struct PaintCanvas<'a> {
    bitmap: Rc<RefCell<Bitmap>>,
    image: Image,
    canvas: OwnedCanvas<'a>,
}

impl PaintCanvas<'_> {

    pub fn new(size: (u32, u32)) -> Self {
        let bitmap = Rc::new(RefCell::new(Bitmap::new()));
        {
            let mut mut_bitmap_ref = bitmap.borrow_mut();
            mut_bitmap_ref.alloc_n32_pixels((size.0 as i32, size.1 as i32), false);
        }
        let bitmap_ref = bitmap.borrow();
        let image = Image::from_bitmap(&bitmap_ref).unwrap();
        Self {
            bitmap: bitmap.clone(),
            image,
            canvas: Canvas::from_bitmap(&bitmap_ref, None),
        }
    }

}
