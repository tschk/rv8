#[cfg(all(target_os = "macos", feature = "servo-render"))]
mod native {
    use std::cell::RefCell;
    use std::os::raw::c_void;
    use std::ptr::null;
    use std::rc::Rc;

    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;
    use core_video::pixel_buffer::{
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange, CVPixelBuffer, CVPixelBufferKeys,
        CVPixelBufferRef,
    };
    use crepuscularity_gpui::Refineable;
    use gpui::{
        size as gpui_size, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId,
        IntoElement, LayoutId, ObjectFit, Pixels, Style, StyleRefinement, Styled, Window,
    };
    #[allow(deprecated)]
    use io_surface::{IOSurface, IOSurfaceGetHeight, IOSurfaceGetWidth};

    type VTPixelTransferSessionRef = *mut c_void;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    #[link(name = "VideoToolbox", kind = "framework")]
    extern "C" {
        fn VTPixelTransferSessionCreate(
            allocator: *const c_void,
            pixel_transfer_session_out: *mut VTPixelTransferSessionRef,
        ) -> i32;
        fn VTPixelTransferSessionTransferImage(
            session: VTPixelTransferSessionRef,
            source_buffer: CVPixelBufferRef,
            destination_buffer: CVPixelBufferRef,
        ) -> i32;
        fn VTPixelTransferSessionInvalidate(session: VTPixelTransferSessionRef);
    }

    const NO_ERR: i32 = 0;

    /// Caches a VideoToolbox pixel-transfer session and a reusable YUV pixel buffer
    /// so converting the IOSurface from BGRA to the YUV format that GPUI expects is
    /// only expensive on the first frame or when the surface size changes.
    pub struct SurfaceConverter {
        session: Option<VTPixelTransferSessionRef>,
        dest: Option<CVPixelBuffer>,
        width: usize,
        height: usize,
    }

    impl SurfaceConverter {
        pub fn new() -> Self {
            Self {
                session: None,
                dest: None,
                width: 0,
                height: 0,
            }
        }

        /// Convert a BGRA IOSurface into a YUV CVPixelBuffer that GPUI can display.
        pub fn convert(&mut self, source: &IOSurface) -> Option<&CVPixelBuffer> {
            let width = unsafe { IOSurfaceGetWidth(source.as_concrete_TypeRef()) };
            let height = unsafe { IOSurfaceGetHeight(source.as_concrete_TypeRef()) };
            tracing::debug!("convert: source {}x{}", width, height);

            if self.session.is_none() {
                let mut session: VTPixelTransferSessionRef = null::<c_void>() as _;
                let status =
                    unsafe { VTPixelTransferSessionCreate(null::<c_void>() as _, &mut session) };
                if status != NO_ERR || session.is_null() {
                    log::error!("VTPixelTransferSessionCreate failed: {status}");
                    return None;
                }
                tracing::debug!("convert: created VTPixelTransferSession");
                self.session = Some(session);
            }

            if self.dest.is_none() || self.width != width || self.height != height {
                let metal_key: CFString = CVPixelBufferKeys::MetalCompatibility.into();
                let options = CFDictionary::from_CFType_pairs(&[(
                    metal_key,
                    CFBoolean::true_value().as_CFType(),
                )]);
                self.dest = CVPixelBuffer::new(
                    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
                    width,
                    height,
                    Some(&options),
                )
                .ok();
                self.width = width;
                self.height = height;
                if self.dest.is_none() {
                    log::error!("Failed to create YUV CVPixelBuffer");
                    return None;
                }
                tracing::debug!("convert: created YUV buffer {}x{}", width, height);
            }

            let src = CVPixelBuffer::from_io_surface(source, None).ok()?;
            let session = self.session.unwrap();
            let dest = self.dest.as_ref().unwrap();
            let status = unsafe {
                VTPixelTransferSessionTransferImage(
                    session,
                    src.as_concrete_TypeRef(),
                    dest.as_concrete_TypeRef(),
                )
            };
            if status != NO_ERR {
                log::error!("VTPixelTransferSessionTransferImage failed: {status}");
                return None;
            }
            tracing::debug!("convert: transferred image");
            self.dest.as_ref()
        }
    }

    impl Drop for SurfaceConverter {
        fn drop(&mut self) {
            if let Some(session) = self.session.take() {
                unsafe {
                    VTPixelTransferSessionInvalidate(session);
                    CFRelease(session as _);
                }
            }
        }
    }

    /// A GPUI element that displays an IOSurface by converting it to the YUV
    /// CVPixelBuffer format that GPUI's Metal renderer expects.
    pub struct NativeSurface {
        source: IOSurface,
        converter: Rc<RefCell<SurfaceConverter>>,
        object_fit: ObjectFit,
        style: StyleRefinement,
    }

    impl NativeSurface {
        pub fn new(source: IOSurface, converter: Rc<RefCell<SurfaceConverter>>) -> Self {
            Self {
                source,
                converter,
                object_fit: ObjectFit::Contain,
                style: StyleRefinement::default(),
            }
        }

        pub fn object_fit(mut self, object_fit: ObjectFit) -> Self {
            self.object_fit = object_fit;
            self
        }
    }

    impl Element for NativeSurface {
        type RequestLayoutState = ();
        type PrepaintState = ();

        fn id(&self) -> Option<ElementId> {
            None
        }

        fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
            None
        }

        fn request_layout(
            &mut self,
            _global_id: Option<&GlobalElementId>,
            _inspector_id: Option<&InspectorElementId>,
            window: &mut Window,
            cx: &mut App,
        ) -> (LayoutId, Self::RequestLayoutState) {
            let mut style = Style::default();
            style.refine(&self.style);
            let layout_id = window.request_layout(style, [], cx);
            (layout_id, ())
        }

        fn prepaint(
            &mut self,
            _global_id: Option<&GlobalElementId>,
            _inspector_id: Option<&InspectorElementId>,
            _bounds: Bounds<Pixels>,
            _request_layout: &mut Self::RequestLayoutState,
            _window: &mut Window,
            _cx: &mut App,
        ) -> Self::PrepaintState {
        }

        fn paint(
            &mut self,
            _global_id: Option<&GlobalElementId>,
            _inspector_id: Option<&InspectorElementId>,
            bounds: Bounds<Pixels>,
            _: &mut Self::RequestLayoutState,
            _: &mut Self::PrepaintState,
            window: &mut Window,
            _: &mut App,
        ) {
            let mut converter = self.converter.borrow_mut();
            if let Some(dest) = converter.convert(&self.source) {
                let size = gpui_size(dest.get_width().into(), dest.get_height().into());
                let new_bounds = self.object_fit.get_bounds(bounds, size);
                window.paint_surface(new_bounds, dest.clone());
            }
        }
    }

    impl IntoElement for NativeSurface {
        type Element = Self;

        fn into_element(self) -> Self::Element {
            self
        }
    }

    impl Styled for NativeSurface {
        fn style(&mut self) -> &mut StyleRefinement {
            &mut self.style
        }
    }
}

#[cfg(all(target_os = "macos", feature = "servo-render"))]
pub use native::{NativeSurface, SurfaceConverter};

#[cfg(not(all(target_os = "macos", feature = "servo-render")))]
pub struct NativeSurface;
