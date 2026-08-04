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
/// The burst erupts from the edges of the "Vote recorded" card.
#[cfg_attr(not(feature = "web"), allow(dead_code))]
const ORIGIN_SELECTOR: &str = ".modal-content";
/// Hard cap on the simulation, and - plus a margin - when `on_done` fires.
/// The two must stay paired: signalling early would cut the burst off
/// mid-air, signalling late leaves a viewport-sized backing store on the
/// compositor for nothing.
const BURST_MS: u32 = 3600;
const TEARDOWN_MS: u32 = BURST_MS + 200;

/// Stash a one-shot cleanup closure inside a signal. `Box<dyn FnOnce()>` is
/// not `Clone`, so it cannot live directly in a `Signal`; the `Rc<RefCell>`
/// wrapper makes it cloneable while still only allowing the inner closure to
/// be taken and called once.
#[derive(Clone)]
struct StopHandle {
    inner: Rc<RefCell<Option<Box<dyn FnOnce()>>>>,
}

impl StopHandle {
    fn new(stop: Box<dyn FnOnce()>) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Some(stop))),
        }
    }

    fn take(&mut self) -> Option<Box<dyn FnOnce()>> {
        self.inner.borrow_mut().take()
    }
}

#[cfg(feature = "web")]
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

#[cfg(feature = "web")]
struct BurstState {
    window: web_sys::Window,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    particles: RefCell<Vec<Particle>>,
    start: f64,
    last: f64,
    max_ms: f64,
    next_raf_id: Cell<Option<i32>>,
    stopped: Cell<bool>,
    animation_closure: RefCell<Option<Closure<dyn FnMut(f64)>>>,
    resize_listener: RefCell<Option<Closure<dyn FnMut()>>>,
}

#[cfg(feature = "web")]
impl BurstState {
    fn new(
        window: web_sys::Window,
        canvas: HtmlCanvasElement,
        ctx: CanvasRenderingContext2d,
        max_ms: f64,
    ) -> Result<Self, JsValue> {
        let state = Self {
            window,
            canvas,
            ctx,
            particles: RefCell::new(Vec::new()),
            start: 0.0,
            last: 0.0,
            max_ms,
            next_raf_id: Cell::new(None),
            stopped: Cell::new(false),
            animation_closure: RefCell::new(None),
            resize_listener: RefCell::new(None),
        };
        state.resize()?;
        Ok(state)
    }

    fn resize(&self) -> Result<(), JsValue> {
        let dpr = self.window.device_pixel_ratio();
        let inner_width = self
            .window
            .inner_width()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let inner_height = self
            .window
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        self.canvas.set_width((inner_width * dpr).max(1.0) as u32);
        self.canvas.set_height((inner_height * dpr).max(1.0) as u32);
        self.canvas
            .style()
            .set_property("width", &format!("{}px", inner_width))?;
        self.canvas
            .style()
            .set_property("height", &format!("{}px", inner_height))?;
        self.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0)?;
        Ok(())
    }

    fn origin_rect(&self) -> Rect {
        let document = self.window.document();
        let origin = document.and_then(|d| d.query_selector(ORIGIN_SELECTOR).ok().flatten());

        if let Some(origin) = origin {
            let rect = origin.get_bounding_client_rect();
            Rect {
                left: rect.left(),
                top: rect.top(),
                width: rect.width(),
                height: rect.height(),
            }
        } else {
            let inner_width = self
                .window
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let inner_height = self
                .window
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            Rect {
                left: inner_width / 2.0,
                top: inner_height / 2.0,
                width: 0.0,
                height: 0.0,
            }
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
        let origin = self.origin_rect();
        let perimeter = 2.0 * (origin.width + origin.height);
        let mut colors = self.colors();
        if colors.is_empty() {
            colors.push("#4f5bd5".to_string());
        }

        let mut particles = self.particles.borrow_mut();
        const COUNT: usize = 140;
        for i in 0..COUNT {
            let d = js_sys::Math::random() * perimeter.max(1.0);

            // Pick a point uniformly along the perimeter, and take that
            // side's outward normal as the base launch direction.
            let (x, y, nx, ny): (f64, f64, f64, f64) = if d < origin.width {
                (origin.left + d, origin.top, 0.0, -1.0)
            } else if d < origin.width + origin.height {
                let d = d - origin.width;
                (origin.left + origin.width, origin.top + d, 1.0, 0.0)
            } else if d < 2.0 * origin.width + origin.height {
                let d = d - origin.width - origin.height;
                (
                    origin.left + origin.width - d,
                    origin.top + origin.height,
                    0.0,
                    1.0,
                )
            } else {
                let d = d - 2.0 * origin.width - origin.height;
                (origin.left, origin.top + origin.height - d, -1.0, 0.0)
            };

            let jitter = (js_sys::Math::random() - 0.5) * (PI / 180.0) * 70.0; // +/-35deg
            let angle = ny.atan2(nx) + jitter;
            let speed = 350.0 + js_sys::Math::random() * 500.0;

            particles.push(Particle {
                x,
                y,
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
}

#[cfg(feature = "web")]
fn start() -> Box<dyn FnOnce()> {
    match try_start() {
        Ok(stop) => stop,
        Err(_) => Box::new(|| {}),
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
        .query_selector(&format!("#{}", CANVAS_ID))?
        .ok_or_else(|| JsValue::from_str("no confetti canvas"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("canvas element is not an HtmlCanvasElement"))?;
    let ctx = canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("no 2d canvas context"))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|_| JsValue::from_str("canvas context is not 2d"))?;

    let state = Rc::new(RefCell::new(BurstState::new(
        window.clone(),
        canvas,
        ctx,
        BURST_MS as f64,
    )?));

    // Resize listener: keep the canvas backing store in sync with the
    // viewport, and keep the closure alive inside the state so it can be
    // detached cleanly.
    let resize_state = state.clone();
    let resize_listener = Closure::new(move || {
        let _ = resize_state.borrow().resize();
    });
    window.add_event_listener_with_callback("resize", resize_listener.as_ref().unchecked_ref())?;
    state
        .borrow()
        .resize_listener
        .replace(Some(resize_listener));

    // Spawn particles from the modal card's perimeter.
    state.borrow().spawn_particles();

    // rAF loop. The closure is stored inside the state so it can be dropped
    // on stop, breaking the Rc cycle it forms with the state.
    let state_for_closure = state.clone();
    let closure = Closure::new(move |now: f64| {
        // Update timing under a short mutable borrow, then release it so the
        // render pass can read the canvas context while mutating particles.
        let (start, max_ms, dt) = {
            let mut state = state_for_closure.borrow_mut();
            if state.stopped.get() {
                return;
            }
            let dt = ((now - state.last) / 1000.0).min(0.05);
            state.last = now;
            (state.start, state.max_ms, dt)
        };

        let mut alive = false;
        {
            let state = state_for_closure.borrow();
            let ctx = &state.ctx;
            let window = &state.window;
            let inner_width = window
                .inner_width()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let inner_height = window
                .inner_height()
                .ok()
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let mut particles = state.particles.borrow_mut();
            ctx.clear_rect(0.0, 0.0, inner_width, inner_height);

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

                if p.y > inner_height + 40.0 {
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
        }

        {
            let state = state_for_closure.borrow_mut();
            if alive && !state.stopped.get() && now - start < max_ms {
                let closure_ref = state.animation_closure.borrow();
                if let Some(closure) = closure_ref.as_ref() {
                    let id = state
                        .window
                        .request_animation_frame(closure.as_ref().unchecked_ref())
                        .unwrap();
                    state.next_raf_id.set(Some(id));
                }
            } else {
                state.stopped.set(true);
            }
        }
    });
    state.borrow_mut().animation_closure.replace(Some(closure));

    // Seed timing and kick off the first frame.
    let now = window
        .performance()
        .ok_or_else(|| JsValue::from_str("no performance"))?
        .now();
    {
        let mut state = state.borrow_mut();
        state.start = now;
        state.last = now;
    }
    let first_id = {
        let state_borrow = state.borrow();
        let closure_ref = state_borrow.animation_closure.borrow();
        let closure = closure_ref
            .as_ref()
            .ok_or_else(|| JsValue::from_str("animation closure missing"))?;
        state_borrow
            .window
            .request_animation_frame(closure.as_ref().unchecked_ref())?
    };
    state.borrow().next_raf_id.set(Some(first_id));

    let state_for_stop = state;
    Ok(Box::new(move || {
        {
            let state = state_for_stop.borrow();
            state.stopped.set(true);
            if let Some(id) = state.next_raf_id.take() {
                let _ = state.window.cancel_animation_frame(id);
            }
            if let Some(resize) = state.resize_listener.borrow_mut().take() {
                let _ = state
                    .window
                    .remove_event_listener_with_callback("resize", resize.as_ref().unchecked_ref());
            }
        }
        // Drop the animation closure, breaking the Rc cycle it forms with
        // the state so everything can be freed.
        state_for_stop.borrow_mut().animation_closure.take();
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
    let mut stop = use_signal(|| StopHandle::new(Box::new(|| {})));

    // Effects never run during SSR, so this only schedules on the client -
    // the same reason `Countdown` can drive itself off a timer. It also runs
    // after the commit, so `.modal-content` is already in the document.
    use_effect(move || {
        let handle = start();
        stop.set(StopHandle::new(handle));
        spawn(async move {
            TimeoutFuture::new(TEARDOWN_MS).await;
            on_done.call(());
        });
    });

    // Only reached by a real unmount - navigating away, or the caller
    // responding to `on_done`. Either way the rAF loop goes with the node.
    use_drop(move || {
        if let Some(stop_fn) = stop.write().take() {
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
