use std::cell::RefCell;
use std::path::{Path, PathBuf};

use objc2::define_class;
use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::AnyThread;
use objc2::MainThreadMarker;
use objc2::MainThreadOnly;
use objc2_app_kit::{
    NSApplication, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession,
    NSDraggingSource, NSEvent, NSEventModifierFlags, NSEventType, NSImage, NSPasteboardWriting,
    NSWorkspace,
};
use objc2_foundation::{NSMutableArray, NSPoint, NSRect, NSString, NSURL};
use pulseseek_browser_fs::drag_out::DragStarter;
use pulseseek_domain::browser::drag_out::DragOutError;

define_class!(
    #[name = "PulseSeekDragSource"]
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    struct DragSource;

    unsafe impl NSObjectProtocol for DragSource {}

    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        #[allow(non_snake_case)]
        fn draggingSession_sourceOperationMaskForDraggingContext(
            &self,
            _session: &NSDraggingSession,
            context: NSDraggingContext,
        ) -> NSDragOperation {
            tracing::info!(?context, "native drag source operation mask requested");
            NSDragOperation::Copy
        }

        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        #[allow(non_snake_case)]
        fn draggingSession_endedAtPoint_operation(
            &self,
            session: &NSDraggingSession,
            _point: NSPoint,
            operation: NSDragOperation,
        ) {
            tracing::info!(
                operation = ?operation,
                accepted = operation != NSDragOperation::None,
                "native drag session ended"
            );
            session.setAnimatesToStartingPositionsOnCancelOrFail(should_animate_back(operation));
        }
    }
);

thread_local! {
    // AppKit starts the actual drag on the next run-loop turn and does not
    // retain the source passed to `beginDraggingSessionWithItems`. Keep one
    // source alive on the main thread so external destinations can negotiate
    // the operation for the complete session.
    static DRAG_SOURCE: RefCell<Option<Retained<ProtocolObject<dyn NSDraggingSource>>>> =
        RefCell::new(None);
}

fn retain_drag_source<T: Clone>(storage: &RefCell<Option<T>>, create: impl FnOnce() -> T) -> T {
    storage.borrow_mut().get_or_insert_with(create).clone()
}

type DragTask = Box<dyn FnOnce() + Send + 'static>;

fn defer_drag<E>(
    paths: Vec<PathBuf>,
    schedule: impl FnOnce(DragTask) -> Result<(), E>,
    run: impl FnOnce(Vec<PathBuf>) + Send + 'static,
) -> Result<(), E> {
    schedule(Box::new(move || run(paths)))
}

impl DragSource {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

/// Native macOS drag-out starter.
///
/// Starts an AppKit drag session on the main window's content view with one
/// pasteboard item per path, each carrying a `public.file-url`. Drop targets
/// (Finder, DAWs, editors) receive a reference to the original file; nothing
/// is copied or written.
pub struct NativeMacosDragStarter {
    app: tauri::AppHandle,
}

impl NativeMacosDragStarter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl DragStarter for NativeMacosDragStarter {
    fn start(&self, paths: &[PathBuf]) -> Result<(), DragOutError> {
        defer_drag(
            paths.to_vec(),
            |task| self.app.run_on_main_thread(task),
            |paths| {
                if let Err(error) = start_drag_session(&paths) {
                    tracing::warn!(
                        error = %error,
                        "native drag session failed to start after scheduling"
                    );
                }
            },
        )
        .map_err(|error| {
            DragOutError::from_io_error(std::io::Error::other(error.to_string()), Path::new(""))
        })
    }
}

fn start_drag_session(paths: &[PathBuf]) -> Result<(), DragOutError> {
    let mtm = MainThreadMarker::new().ok_or_else(DragOutError::unsupported)?;
    let app = NSApplication::sharedApplication(mtm);
    let window = app.mainWindow().ok_or_else(DragOutError::unsupported)?;
    let content_view = window.contentView().ok_or_else(DragOutError::unsupported)?;

    let current_position = window.mouseLocationOutsideOfEventStream();
    let image = NSWorkspace::sharedWorkspace()
        .iconForFile(&NSString::from_str(&paths[0].to_string_lossy()));
    let image_size = image.size();
    let image_rect = NSRect::new(
        NSPoint::new(
            current_position.x - image_size.width / 2.0,
            current_position.y - image_size.height / 2.0,
        ),
        image_size,
    );

    let items = build_dragging_items(paths, &image, image_rect)?;
    let event = build_mouse_event(&window, &app, current_position);
    let source = DRAG_SOURCE.with(|storage| {
        retain_drag_source(storage, || {
            ProtocolObject::<dyn NSDraggingSource>::from_retained(DragSource::new(mtm))
        })
    });
    let session = content_view.beginDraggingSessionWithItems_event_source(&items, &event, &source);
    let pasteboard_types = session
        .draggingPasteboard()
        .types()
        .map(|types| {
            types.iter().map(|pasteboard_type| pasteboard_type.to_string()).collect::<Vec<_>>()
        })
        .unwrap_or_default();
    tracing::info!(?pasteboard_types, "native drag session started");
    Ok(())
}

fn build_dragging_items(
    paths: &[PathBuf],
    image: &NSImage,
    image_rect: NSRect,
) -> Result<Retained<NSMutableArray<NSDraggingItem>>, DragOutError> {
    let items = NSMutableArray::array();
    for path in paths {
        let path_string = NSString::from_str(&path.to_string_lossy());
        let url = NSURL::fileURLWithPath_isDirectory(&path_string, false);
        let writer = ProtocolObject::<dyn NSPasteboardWriting>::from_retained(url);
        let drag_item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), &writer);
        // SAFETY: `image` is a valid NSImage that outlives this call.
        unsafe { drag_item.setDraggingFrame_contents(image_rect, Some(image)) };
        items.addObject(&*drag_item);
    }
    Ok(items)
}

fn should_animate_back(operation: NSDragOperation) -> bool {
    operation == NSDragOperation::None
}

fn build_mouse_event(
    window: &objc2_app_kit::NSWindow,
    app: &NSApplication,
    location: NSPoint,
) -> Retained<NSEvent> {
    let timestamp = app.currentEvent().map(|e| e.timestamp()).unwrap_or(0.0);
    NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDragged,
        location,
        NSEventModifierFlags::empty(),
        timestamp,
        window.windowNumber(),
        None,
        0,
        1,
        1.0,
    )
    .expect("failed to create mouse event")
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::*;

    struct DropState(Rc<Cell<usize>>);

    impl Drop for DropState {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[derive(Clone)]
    struct DropProbe {
        _state: Rc<DropState>,
    }

    #[test]
    fn native_drag_waits_for_the_next_event_loop_turn() {
        let ran = Arc::new(AtomicBool::new(false));
        let queued = Arc::new(Mutex::new(None::<DragTask>));
        let queued_for_scheduler = Arc::clone(&queued);
        let ran_for_task = Arc::clone(&ran);

        defer_drag(
            vec![PathBuf::from("/music/a.wav")],
            move |task| {
                *queued_for_scheduler.lock().unwrap() = Some(task);
                Ok::<(), ()>(())
            },
            move |paths| {
                assert_eq!(paths, vec![PathBuf::from("/music/a.wav")]);
                ran_for_task.store(true, Ordering::Release);
            },
        )
        .unwrap();

        assert!(!ran.load(Ordering::Acquire));
        queued.lock().unwrap().take().unwrap()();
        assert!(ran.load(Ordering::Acquire));
    }

    #[test]
    fn drag_source_outlives_the_local_begin_session_handle() {
        let drops = Rc::new(Cell::new(0));
        let storage = std::cell::RefCell::new(None);

        let local_handle = retain_drag_source(&storage, || DropProbe {
            _state: Rc::new(DropState(Rc::clone(&drops))),
        });
        drop(local_handle);

        assert_eq!(drops.get(), 0, "the session source must remain retained");
        drop(storage);
        assert_eq!(drops.get(), 1, "the retained source is eventually released");
    }

    #[test]
    fn accepted_drop_does_not_animate_back_to_pulseseek() {
        assert!(!should_animate_back(NSDragOperation::Copy));
    }

    #[test]
    fn cancelled_drop_animates_back_to_pulseseek() {
        assert!(should_animate_back(NSDragOperation::None));
    }
}
