//! Native Pango shapes preserve TeX characters, selection and link offsets.
use glib::translate::*;
use gtk::{cairo, glib, pango, prelude::*};
use std::{
    cell::Cell,
    ffi::c_void,
    ops::Range,
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
pub const IMAGE_BYTES: usize = 2 * 1024 * 1024;
pub const SURFACE_BYTES: usize = 16 * 1024 * 1024;

#[link(name = "pangocairo-1.0")]
unsafe extern "C" {
    fn pango_cairo_context_set_shape_renderer(
        context: *mut pango::ffi::PangoContext,
        renderer: Option<
            unsafe extern "C" fn(
                *mut cairo::ffi::cairo_t,
                *mut pango::ffi::PangoAttrShape,
                glib::ffi::gboolean,
                *mut c_void,
            ),
        >,
        data: *mut c_void,
        destroy: glib::ffi::GDestroyNotify,
    );
}
pub struct Prepared {
    pub metrics: crate::math::Metrics,
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}
impl Prepared {
    /// Run beside rasterization on the Rust worker, not during GTK layout.
    pub fn new(image: crate::diagram::Image) -> Result<Self, String> {
        if image.pixels.len() > IMAGE_BYTES {
            return Err("Formula raster is too large for inline display".into());
        }
        let metrics = image.math.ok_or("Inline image has no math metrics")?;
        let mut pixels = image.pixels.to_vec();
        for pixel in pixels.chunks_exact_mut(4) {
            let argb = u32::from_be_bytes([pixel[3], pixel[0], pixel[1], pixel[2]]).to_ne_bytes();
            pixel.copy_from_slice(&argb);
        }
        Ok(Self {
            metrics,
            width: image.width,
            height: image.height,
            pixels,
        })
    }
}
pub struct Surface {
    metrics: crate::math::Metrics,
    image: cairo::ImageSurface,
    pub bytes: usize,
}
struct Pixels {
    data: Vec<u8>,
    budget: Option<Arc<AtomicUsize>>,
}
impl AsMut<[u8]> for Pixels {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}
impl Drop for Pixels {
    fn drop(&mut self) {
        if let Some(budget) = &self.budget {
            budget.fetch_sub(self.data.len(), Ordering::AcqRel);
        }
    }
}
impl Surface {
    pub fn new(image: Prepared, budget: Option<Arc<AtomicUsize>>) -> Result<Rc<Self>, String> {
        if image.width == 0
            || image.height == 0
            || image.width > 2048
            || image.height > 4096
            || image.pixels.len() != image.width as usize * image.height as usize * 4
            || image.pixels.len() > IMAGE_BYTES
        {
            return Err("Invalid inline image bounds".into());
        }
        let bytes = image.pixels.len();
        if let Some(budget) = &budget {
            budget
                .fetch_update(Ordering::AcqRel, Ordering::Relaxed, |used| {
                    used.checked_add(bytes)
                        .filter(|total| *total <= SURFACE_BYTES)
                })
                .map_err(|_| "Too much inline math is visible; original TeX is retained")?;
        }
        // The charge follows Cairo's pixel allocation, including snapshots that
        // outlive the Rust wrapper. Its destructor may run outside the GUI lane.
        let data = Pixels {
            data: image.pixels,
            budget,
        };
        let surface = cairo::ImageSurface::create_for_data(
            data,
            cairo::Format::ARgb32,
            image.width as i32,
            image.height as i32,
            image.width as i32 * 4,
        )
        .map_err(|e| e.to_string())?;
        Ok(Rc::new(Self {
            metrics: image.metrics,
            image: surface,
            bytes,
        }))
    }
}
struct Shape {
    surface: Rc<Surface>,
    draws: Rc<Cell<usize>>,
}
unsafe extern "C" fn copy(data: *const c_void) -> *mut c_void {
    // Pango copies its owned attribute data; every copy owns one Rc reference.
    unsafe {
        Rc::<Shape>::increment_strong_count(data.cast());
    }
    data.cast_mut()
}
unsafe extern "C" fn destroy(data: *mut c_void) {
    unsafe {
        drop(Rc::<Shape>::from_raw(data.cast()));
    }
}
unsafe extern "C" fn draw(
    raw: *mut cairo::ffi::cairo_t,
    attribute: *mut pango::ffi::PangoAttrShape,
    path: glib::ffi::gboolean,
    _: *mut c_void,
) {
    if raw.is_null() || attribute.is_null() {
        return;
    }
    let attribute = unsafe { &*attribute };
    if attribute.data.is_null() {
        return;
    }
    let shape = unsafe { &*attribute.data.cast::<Shape>() };
    let context = unsafe { cairo::Context::from_raw_none(raw) };
    let Ok((x, y)) = context.current_point() else {
        return;
    };
    let unit = f64::from(pango::SCALE);
    let logical = &attribute.logical_rect;
    let (x, y) = (
        x + f64::from(logical.x) / unit,
        y + f64::from(logical.y) / unit,
    );
    let (width, height) = (
        f64::from(logical.width) / unit,
        f64::from(logical.height) / unit,
    );
    if path != 0 {
        context.rectangle(x, y, width, height);
        return;
    }
    if context.save().is_err() {
        return;
    }
    context.translate(x, y);
    context.scale(
        width / f64::from(shape.surface.image.width()),
        height / f64::from(shape.surface.image.height()),
    );
    context.rectangle(
        0.0,
        0.0,
        f64::from(shape.surface.image.width()),
        f64::from(shape.surface.image.height()),
    );
    context.clip();
    if context
        .set_source_surface(&shape.surface.image, 0.0, 0.0)
        .is_ok()
    {
        if context.paint().is_ok() {
            shape.draws.set(shape.draws.get().saturating_add(1));
        }
    }
    let _ = context.restore();
}

#[allow(dead_code)] // The standalone layout probe uses an explicit font size.
pub fn apply(
    label: &gtk::Label,
    formulas: Vec<(Range<usize>, Prepared)>,
    font: f64,
) -> Result<Rc<Cell<usize>>, String> {
    let surfaces = formulas
        .into_iter()
        .map(|(range, image)| Surface::new(image, None).map(|surface| (range, surface)))
        .collect::<Result<Vec<_>, _>>()?;
    apply_surfaces(label, &surfaces, font, None, true)
}
pub fn apply_surfaces(
    label: &gtk::Label,
    formulas: &[(Range<usize>, Rc<Surface>)],
    font: f64,
    base: Option<&pango::AttrList>,
    force_font: bool,
) -> Result<Rc<Cell<usize>>, String> {
    if !font.is_finite() || !(8.0..=96.0).contains(&font) || formulas.len() > 256 {
        return Err("Inline shape request exceeds its bounds".into());
    }
    let text = label.text();
    if text.len() > 512 * 1024 {
        return Err("Inline paragraph exceeds its budget".into());
    }
    let attrs = base.and_then(|attrs| attrs.copy()).unwrap_or_default();
    if force_font {
        attrs.insert(pango::AttrSize::new_size_absolute(
            (font * f64::from(pango::SCALE)).round() as i32,
        ));
    }
    let draws = Rc::new(Cell::new(0));
    for (range, image) in formulas {
        let raw = text
            .get(range.clone())
            .filter(|s| s.starts_with('$'))
            .ok_or("Math range differs from label text")?;
        if raw.is_empty() {
            return Err("Invalid math range".into());
        }
        let factor = font / image.metrics.em * f64::from(pango::SCALE);
        if [
            image.metrics.width,
            image.metrics.height,
            image.metrics.baseline,
            factor,
        ]
        .iter()
        .any(|n| !n.is_finite())
            || factor <= 0.0
            || image.metrics.width <= 0.0
            || image.metrics.height <= 0.0
            || [
                image.metrics.width,
                image.metrics.height,
                image.metrics.baseline.abs(),
            ]
            .iter()
            .any(|n| n * factor > 8192.0 * f64::from(pango::SCALE))
        {
            return Err("Inline layout exceeds its bounds".into());
        }
        let rectangle = pango::Rectangle::new(
            0,
            (-image.metrics.baseline * factor).round() as i32,
            (image.metrics.width * factor).ceil() as i32,
            (image.metrics.height * factor).ceil() as i32,
        );
        let shape = Rc::new(Shape {
            surface: image.clone(),
            draws: draws.clone(),
        });
        // One visible shape uses the opening ASCII dollar. Remaining source
        // characters get zero-width shapes, so underlying text never changes.
        let mut visible: pango::Attribute = unsafe {
            from_glib_full(pango::ffi::pango_attr_shape_new_with_data(
                rectangle.to_glib_none().0,
                rectangle.to_glib_none().0,
                Rc::into_raw(shape).cast_mut().cast(),
                Some(copy),
                Some(destroy),
            ))
        };
        visible.set_start_index(range.start as u32);
        visible.set_end_index(range.start as u32 + 1);
        attrs.insert(visible);
        let empty = pango::Rectangle::new(0, 0, 0, 0);
        let mut hidden = pango::AttrShape::new(&empty, &empty);
        hidden.set_start_index(range.start as u32 + 1);
        hidden.set_end_index(range.end as u32);
        attrs.insert(hidden);
        for mut group in [
            pango::AttrInt::new_allow_breaks(false),
            pango::AttrInt::new_word(),
        ] {
            group.set_start_index(range.start as u32);
            group.set_end_index(range.end as u32);
            attrs.insert(group);
        }
    }
    unsafe {
        pango_cairo_context_set_shape_renderer(
            label.pango_context().to_glib_none().0,
            Some(draw),
            std::ptr::null_mut(),
            None,
        );
    }
    let selection = label.selection_bounds();
    label.set_attributes(Some(&attrs));
    if let Some((start, end)) = selection {
        label.select_region(start, end);
    }
    Ok(draws)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> Prepared {
        Prepared {
            metrics: crate::math::Metrics {
                width: 1.0,
                height: 1.0,
                baseline: 0.8,
                em: 24.0,
            },
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        }
    }
    #[test]
    fn pixel_budget_follows_cairo_references_after_wrapper_drop() {
        let budget = Arc::new(AtomicUsize::new(SURFACE_BYTES - 4));
        let surface = Surface::new(image(), Some(budget.clone())).unwrap();
        assert_eq!(budget.load(Ordering::Acquire), SURFACE_BYTES);
        let snapshot = surface.image.clone();
        drop(surface);
        assert_eq!(budget.load(Ordering::Acquire), SURFACE_BYTES);
        assert!(Surface::new(image(), Some(budget.clone())).is_err());
        drop(snapshot);
        assert_eq!(budget.load(Ordering::Acquire), SURFACE_BYTES - 4);
    }
}
