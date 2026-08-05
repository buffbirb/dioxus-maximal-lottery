//! One-shot confetti burst, mounted for the moment a vote lands. Purely
//! decorative: `aria-hidden` and `pointer-events: none` keep it out of the
//! accessibility tree and out of the way of the modal it plays over. Its
//! lifetime is owned by the caller via `on_done`, not by the modal it
//! erupts from - closing "Vote recorded" early must not cut the burst off.
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "web")]
use std::cell::Cell;
#[cfg(feature = "web")]
use std::f64::consts::PI;

#[cfg(feature = "web")]
use wasm_bindgen::JsCast;
#[cfg(feature = "web")]
use wasm_bindgen::prelude::*;
#[cfg(feature = "web")]
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, window};

const CANVAS_ID: &str = "confetti-canvas";
/// The burst is centred on the "Vote recorded" card.
#[cfg_attr(not(feature = "web"), allow(dead_code))]
const ORIGIN_SELECTOR: &str = ".modal-content";
/// Hard cap on the simulation, and - plus a margin - when `on_done` fires.
/// The two must stay paired: signalling early would cut the burst off
/// mid-air, signalling late leaves a viewport-sized backing store on the
/// compositor for nothing.
const BURST_MS: u32 = 3600;
const TEARDOWN_MS: u32 = BURST_MS + 200;

type StopFn = Box<dyn FnOnce()>;

/// One-shot cleanup handle. A `StopFn` can be neither cloned nor called from
/// behind a shared reference, so it lives behind an `Rc<RefCell<Option<_>>>`:
/// the effect and the drop handler each hold a clone, and whichever reaches
/// it first takes the closure out and runs it.
#[derive(Clone, Default)]
struct StopHandle(Rc<RefCell<Option<StopFn>>>);

impl StopHandle {
    fn set(&self, stop: StopFn) {
        *self.0.borrow_mut() = Some(stop);
    }

    fn take(&self) -> Option<StopFn> {
        self.0.borrow_mut().take()
    }
}

/// The rAF callback, which receives the frame's `DOMHighResTimeStamp`.
#[cfg(feature = "web")]
type FrameCallback = Closure<dyn FnMut(f64)>;

#[cfg(feature = "web")]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
}

#[cfg(feature = "web")]
struct Particle {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    gravity: f64,
    drag: f64,
    w: f64,
    h: f64,
    color: String,
    rotation: f64,
    spin: f64,
    tumble_phase: f64,
    tumble_speed: f64,
    life: f64,
    age: f64,
}

/// Every field is interior-mutable so the state can live in a plain `Rc` and
/// be reached from the rAF closure, the resize listener and the stop handle
/// at once, without any of them having to sequence borrows against the
/// others.
#[cfg(feature = "web")]
struct BurstState {
    window: web_sys::Window,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    particles: RefCell<Vec<Particle>>,
    /// The canvas's own box in viewport coordinates, remeasured on resize.
    /// Particles live in canvas-local space, so this is what converts the
    /// viewport-relative rects the DOM hands back; see `resize`.
    view: Cell<Rect>,
    start: Cell<f64>,
    last: Cell<f64>,
    next_raf_id: Cell<Option<i32>>,
    stopped: Cell<bool>,
    animation_closure: RefCell<Option<FrameCallback>>,
    resize_listener: RefCell<Option<Closure<dyn FnMut()>>>,
}

#[cfg(feature = "web")]
impl BurstState {
    fn new(
        window: web_sys::Window,
        canvas: HtmlCanvasElement,
        ctx: CanvasRenderingContext2d,
        now: f64,
    ) -> Result<Self, JsValue> {
        let state = Self {
            window,
            canvas,
            ctx,
            particles: RefCell::new(Vec::new()),
            view: Cell::new(Rect::default()),
            start: Cell::new(now),
            last: Cell::new(now),
            next_raf_id: Cell::new(None),
            stopped: Cell::new(false),
            animation_closure: RefCell::new(None),
            resize_listener: RefCell::new(None),
        };
        state.resize()?;
        Ok(state)
    }

    /// Size the backing store to the canvas's *measured* box rather than to
    /// the viewport.
    ///
    /// Drawing happens in canvas-local coordinates, but everything the burst
    /// aims at is measured with `getBoundingClientRect`, which is relative to
    /// the viewport. Deriving the surface from `window.innerWidth/Height`
    /// instead would silently assume the canvas's top-left sits exactly at
    /// the viewport's - an assumption the stylesheet, a scrollbar (Firefox
    /// counts one in `innerWidth`, the fixed box does not) or anything that
    /// knocks the element out of `position: fixed` can all break, and which
    /// shows up as the whole burst drawn at a constant offset. Measuring the
    /// element and subtracting its origin cannot be wrong about where it is.
    ///
    /// Note this deliberately does not write the size back as inline CSS: the
    /// stylesheet lays the canvas out, and `set_width`/`set_height` only
    /// touch the backing store, so there is no feedback loop between the two.
    fn resize(&self) -> Result<(), JsValue> {
        let dpr = self.window.device_pixel_ratio().max(1.0);
        let rect = self.canvas.get_bounding_client_rect();
        let width = rect.width().max(1.0);
        let height = rect.height().max(1.0);

        self.canvas.set_width((width * dpr) as u32);
        self.canvas.set_height((height * dpr) as u32);
        self.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0)?;
        self.view.set(Rect {
            left: rect.left(),
            top: rect.top(),
            width,
            height,
        });
        Ok(())
    }

    /// Centre of the "Vote recorded" card, in canvas-local coordinates.
    /// Falls back to the middle of the canvas if the card isn't there for
    /// some reason.
    fn origin_center(&self) -> (f64, f64) {
        let view = self.view.get();
        let card = self
            .window
            .document()
            .and_then(|d| d.query_selector(ORIGIN_SELECTOR).ok().flatten())
            .map(|el| el.get_bounding_client_rect());

        match card {
            Some(rect) => (
                rect.left() + rect.width() / 2.0 - view.left,
                rect.top() + rect.height() / 2.0 - view.top,
            ),
            None => (view.width / 2.0, view.height / 2.0),
        }
    }

    fn colors(&self) -> Vec<String> {
        let Some(document) = self.window.document() else {
            return Vec::new();
        };
        let Some(document_element) = document.document_element() else {
            return Vec::new();
        };
        let Ok(Some(style)) = self.window.get_computed_style(&document_element) else {
            return Vec::new();
        };

        ["--accent", "--success", "--danger", "--accent-hover"]
            .iter()
            .filter_map(|name| style.get_property_value(name).ok())
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect()
    }

    fn spawn_particles(&self) {
        let (cx, cy) = self.origin_center();
        let mut colors = self.colors();
        if colors.is_empty() {
            colors.push("#4f5bd5".to_string());
        }

        let mut particles = self.particles.borrow_mut();
        const COUNT: usize = 140;
        /// Launch directions fan across 135 degrees centred straight up, so
        /// even the outermost pieces still leave at a diagonal rather than
        /// flat out to the sides. Widen this to spend more of the burst
        /// sideways; narrow it for a tighter column.
        const SPREAD: f64 = PI * 0.75;
        for i in 0..COUNT {
            // Everything leaves the same point at the centre of the card.
            // Straight up is `-PI / 2`, canvas y growing downward; gravity is
            // what turns the throw into a fall a moment later.
            let angle = -PI / 2.0 + (js_sys::Math::random() - 0.5) * SPREAD;
            let speed = 550.0 + js_sys::Math::random() * 800.0;

            particles.push(Particle {
                x: cx,
                y: cy,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed,
                gravity: 900.0 + js_sys::Math::random() * 500.0,
                drag: 0.15 + js_sys::Math::random() * 0.15,
                w: 6.0 + js_sys::Math::random() * 5.0,
                h: 8.0 + js_sys::Math::random() * 8.0,
                color: colors[i % colors.len()].clone(),
                rotation: js_sys::Math::random() * PI * 2.0,
                spin: (js_sys::Math::random() - 0.5) * 10.0,
                tumble_phase: js_sys::Math::random() * PI * 2.0,
                tumble_speed: 4.0 + js_sys::Math::random() * 6.0,
                life: 1.6 + js_sys::Math::random() * 1.4,
                age: 0.0,
            });
        }
    }

    /// Queue the next frame. `false` means there is nothing left to queue -
    /// the caller turns that into a teardown rather than stalling with the
    /// loop half-alive.
    fn schedule_frame(&self) -> bool {
        let closure = self.animation_closure.borrow();
        let Some(closure) = closure.as_ref() else {
            return false;
        };
        match self
            .window
            .request_animation_frame(closure.as_ref().unchecked_ref())
        {
            Ok(id) => {
                self.next_raf_id.set(Some(id));
                true
            }
            Err(_) => false,
        }
    }

    /// Stop the loop and hand back everything the browser is holding on our
    /// behalf.
    ///
    /// Safe to call from inside the rAF closure, which is why it pointedly
    /// leaves `animation_closure` alone: dropping a `Closure` from within its
    /// own body would free the code that is currently running. Only `stop`
    /// below - which is only ever reached from `use_drop` - may do that, and
    /// only after the pending frame has been cancelled.
    fn detach(&self) {
        self.stopped.set(true);
        if let Some(id) = self.next_raf_id.take() {
            let _ = self.window.cancel_animation_frame(id);
        }
        if let Some(resize) = self.resize_listener.borrow_mut().take() {
            let _ = self
                .window
                .remove_event_listener_with_callback("resize", resize.as_ref().unchecked_ref());
        }
    }
}

#[cfg(feature = "web")]
fn start() -> Box<dyn FnOnce()> {
    match try_start() {
        Ok(stop) => stop,
        Err(err) => {
            // Decorative, so a failure is never worth breaking the page over -
            // but it is worth saying out loud, since every `Err` on the way
            // here carries a specific reason the burst didn't happen.
            web_sys::console::warn_2(&JsValue::from_str("confetti:"), &err);
            Box::new(|| {})
        }
    }
}

#[cfg(feature = "web")]
fn try_start() -> Result<Box<dyn FnOnce()>, JsValue> {
    let window = window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;

    let reduced_motion = window
        .match_media("(prefers-reduced-motion: reduce)")?
        .map(|m| m.matches())
        .unwrap_or(false);
    if reduced_motion {
        return Ok(Box::new(|| {}));
    }

    let canvas = document
        .get_element_by_id(CANVAS_ID)
        .ok_or_else(|| JsValue::from_str("no confetti canvas"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("canvas element is not an HtmlCanvasElement"))?;
    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d canvas context"))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|_| JsValue::from_str("canvas context is not 2d"))?;

    // Read the clock before anything is attached to the window, so that every
    // fallible step left below this point is one we can unwind cleanly.
    let now = window
        .performance()
        .ok_or_else(|| JsValue::from_str("no performance"))?
        .now();

    let state = Rc::new(BurstState::new(window.clone(), canvas, ctx, now)?);

    // Keep the backing store - and the viewport origin the burst is drawn
    // against - in step with the canvas, and keep the closure alive inside
    // the state so it can be detached again cleanly.
    let resize_state = state.clone();
    let resize_listener: Closure<dyn FnMut()> = Closure::new(move || {
        let _ = resize_state.resize();
    });
    window.add_event_listener_with_callback("resize", resize_listener.as_ref().unchecked_ref())?;
    state.resize_listener.replace(Some(resize_listener));

    state.spawn_particles();

    // rAF loop. The closure is stored inside the state so it can be dropped
    // on stop, breaking the Rc cycle it forms with the state.
    let frame_state = state.clone();
    let closure: FrameCallback = Closure::new(move |now: f64| {
        let state = &frame_state;
        if state.stopped.get() {
            return;
        }

        let dt = ((now - state.last.get()) / 1000.0).min(0.05);
        state.last.set(now);

        let view = state.view.get();
        let ctx = &state.ctx;
        let mut particles = state.particles.borrow_mut();
        let mut alive = false;

        ctx.clear_rect(0.0, 0.0, view.width, view.height);

        for p in particles.iter_mut() {
            p.age += dt;
            if p.age >= p.life {
                continue;
            }

            p.vx *= (1.0 - p.drag * dt).max(0.0);
            p.vy += p.gravity * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.rotation += p.spin * dt;
            p.tumble_phase += p.tumble_speed * dt;

            if p.y > view.height + 40.0 {
                continue;
            }
            alive = true;

            let fade_start = p.life - 0.4;
            let opacity = if p.age > fade_start {
                (1.0 - (p.age - fade_start) / 0.4).max(0.0)
            } else {
                1.0
            };
            let scale_y = p.tumble_phase.cos().abs();

            ctx.save();
            ctx.set_global_alpha(opacity);
            let _ = ctx.translate(p.x, p.y);
            let _ = ctx.rotate(p.rotation);
            let _ = ctx.scale(1.0, scale_y.max(0.15));
            ctx.set_fill_style_str(&p.color);
            ctx.fill_rect(-p.w / 2.0, -p.h / 2.0, p.w, p.h);
            ctx.restore();
        }
        drop(particles);

        if !(alive && now - state.start.get() < BURST_MS as f64 && state.schedule_frame()) {
            state.detach();
        }
    });
    state.animation_closure.replace(Some(closure));

    if !state.schedule_frame() {
        // The listener and the closure are both attached by now, and nothing
        // is going to come back to collect them, so unwind by hand.
        state.detach();
        state.animation_closure.borrow_mut().take();
        return Err(JsValue::from_str("could not schedule the first frame"));
    }

    let stop_state = state;
    Ok(Box::new(move || {
        stop_state.detach();
        // Safe only here, never from inside the loop: dropping the closure
        // breaks the Rc cycle it forms with the state, so everything else can
        // be freed with it.
        stop_state.animation_closure.borrow_mut().take();
    }))
}

#[cfg(not(feature = "web"))]
fn start() -> Box<dyn FnOnce()> {
    Box::new(|| {})
}

/// `on_done` fires once the last piece has landed, for the caller to unmount
/// with. The burst does not unmount itself: the caller decides when it is
/// over, which is what lets it outlive the modal it erupted from.
#[component]
pub fn Confetti(on_done: EventHandler<()>) -> Element {
    let stop = use_hook(StopHandle::default);

    // Effects never run during SSR, so this only schedules on the client -
    // the same reason `Countdown` can drive itself off a timer. It also runs
    // after the commit, so `.modal-content` is already in the document.
    use_effect({
        let stop = stop.clone();
        move || {
            // Nothing re-runs this effect today, but a re-run that stranded
            // the previous loop would leave it running with no handle left to
            // stop it, so retire one before taking another.
            if let Some(previous) = stop.take() {
                previous();
            }
            stop.set(start());
            spawn(async move {
                TimeoutFuture::new(TEARDOWN_MS).await;
                on_done.call(());
            });
        }
    });

    // Only reached by a real unmount - navigating away, or the caller
    // responding to `on_done`. Either way the rAF loop goes with the node.
    use_drop(move || {
        if let Some(stop_fn) = stop.take() {
            stop_fn();
        }
    });

    rsx! {
        canvas {
            id: "{CANVAS_ID}",
            class: "confetti-canvas",
            "aria-hidden": "true",
        }
    }
}
