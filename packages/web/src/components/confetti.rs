//! One-shot confetti burst, mounted for the moment a vote lands. Purely
//! decorative: `aria-hidden` and `pointer-events: none` keep it out of the
//! accessibility tree and out of the way of the modal it plays over. Its
//! lifetime is owned by the caller via `on_done`, not by the modal it
//! erupts from - closing "Vote recorded" early must not cut the burst off.
use dioxus::prelude::*;
use gloo_timers::future::TimeoutFuture;

#[cfg(feature = "web")]
mod ffi {
    use wasm_bindgen::prelude::*;

    // Plain synchronous FFI (see tier_ranker.rs for the precedent): the
    // burst needs to measure `.modal-content`'s rect before the modal has a
    // chance to disappear, which `document::eval` can't guarantee.
    #[wasm_bindgen(inline_js = r#"
        let state = null;

        function stopLoop() {
            if (!state) return;
            cancelAnimationFrame(state.raf);
            window.removeEventListener('resize', state.onResize);
            state = null;
        }

        export function confetti_stop() {
            stopLoop();
        }

        export function confetti_start(canvasSelector, originSelector, maxMs) {
            stopLoop();

            if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
                return;
            }

            const canvas = document.querySelector(canvasSelector);
            if (!canvas) return;
            const ctx = canvas.getContext('2d');
            if (!ctx) return;

            const style = getComputedStyle(document.documentElement);
            const colors = [
                style.getPropertyValue('--accent').trim(),
                style.getPropertyValue('--success').trim(),
                style.getPropertyValue('--danger').trim(),
                style.getPropertyValue('--accent-hover').trim(),
            ].filter(Boolean);
            if (colors.length === 0) colors.push('#4f5bd5');

            function resize() {
                const dpr = window.devicePixelRatio || 1;
                canvas.width = window.innerWidth * dpr;
                canvas.height = window.innerHeight * dpr;
                canvas.style.width = window.innerWidth + 'px';
                canvas.style.height = window.innerHeight + 'px';
                ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
            }
            resize();
            const onResize = () => resize();
            window.addEventListener('resize', onResize);

            // Emit from the perimeter of the modal card, so the burst reads
            // as erupting out from behind it. Falls back to viewport centre
            // if the card isn't there for some reason.
            const origin = document.querySelector(originSelector);
            const rect = origin
                ? origin.getBoundingClientRect()
                : { left: window.innerWidth / 2, top: window.innerHeight / 2, width: 0, height: 0 };

            const perimeter = 2 * (rect.width + rect.height);
            const pieces = [];
            const count = 140;
            for (let i = 0; i < count; i++) {
                // Pick a point uniformly along the perimeter, and take that
                // side's outward normal as the base launch direction.
                let d = Math.random() * (perimeter || 1);
                let x, y, nx, ny;
                if (d < rect.width) {
                    x = rect.left + d; y = rect.top; nx = 0; ny = -1;
                } else if (d < rect.width + rect.height) {
                    d -= rect.width;
                    x = rect.left + rect.width; y = rect.top + d; nx = 1; ny = 0;
                } else if (d < 2 * rect.width + rect.height) {
                    d -= rect.width + rect.height;
                    x = rect.left + rect.width - d; y = rect.top + rect.height; nx = 0; ny = 1;
                } else {
                    d -= 2 * rect.width + rect.height;
                    x = rect.left; y = rect.top + rect.height - d; nx = -1; ny = 0;
                }

                const jitter = (Math.random() - 0.5) * (Math.PI / 180) * 70; // +/-35deg
                const angle = Math.atan2(ny, nx) + jitter;
                const speed = 350 + Math.random() * 500;

                pieces.push({
                    x, y,
                    vx: Math.cos(angle) * speed,
                    vy: Math.sin(angle) * speed,
                    gravity: 900 + Math.random() * 500,
                    drag: 0.15 + Math.random() * 0.15,
                    w: 6 + Math.random() * 5,
                    h: 8 + Math.random() * 8,
                    color: colors[i % colors.length],
                    rotation: Math.random() * Math.PI * 2,
                    spin: (Math.random() - 0.5) * 10,
                    tumblePhase: Math.random() * Math.PI * 2,
                    tumbleSpeed: 4 + Math.random() * 6,
                    life: 1.6 + Math.random() * 1.4,
                    age: 0,
                });
            }

            const start = performance.now();
            let last = start;

            function frame(now) {
                const dt = Math.min((now - last) / 1000, 0.05);
                last = now;

                ctx.clearRect(0, 0, window.innerWidth, window.innerHeight);

                let alive = false;
                for (const p of pieces) {
                    p.age += dt;
                    if (p.age >= p.life) continue;

                    p.vx *= Math.max(0, 1 - p.drag * dt);
                    p.vy += p.gravity * dt;
                    p.x += p.vx * dt;
                    p.y += p.vy * dt;
                    p.rotation += p.spin * dt;
                    p.tumblePhase += p.tumbleSpeed * dt;

                    if (p.y > window.innerHeight + 40) continue;
                    alive = true;

                    const fadeStart = p.life - 0.4;
                    const opacity = p.age > fadeStart
                        ? Math.max(0, 1 - (p.age - fadeStart) / 0.4)
                        : 1;

                    const scaleY = Math.abs(Math.cos(p.tumblePhase));

                    ctx.save();
                    ctx.globalAlpha = opacity;
                    ctx.translate(p.x, p.y);
                    ctx.rotate(p.rotation);
                    ctx.scale(1, Math.max(0.15, scaleY));
                    ctx.fillStyle = p.color;
                    ctx.fillRect(-p.w / 2, -p.h / 2, p.w, p.h);
                    ctx.restore();
                }

                if (alive && now - start < maxMs && state) {
                    state.raf = requestAnimationFrame(frame);
                } else {
                    stopLoop();
                }
            }

            state = { raf: requestAnimationFrame(frame), onResize };
        }
    "#)]
    extern "C" {
        pub fn confetti_start(canvas: &str, origin: &str, max_ms: f64);
        pub fn confetti_stop();
    }
}

#[cfg_attr(not(feature = "web"), allow(dead_code))]
const CANVAS_SELECTOR: &str = "#confetti-canvas";
/// The burst erupts from the edges of the "Vote recorded" card.
#[cfg_attr(not(feature = "web"), allow(dead_code))]
const ORIGIN_SELECTOR: &str = ".modal-content";

/// Hard cap on the simulation, and - plus a margin - when `on_done` fires.
/// The two must stay paired: signalling early would cut the burst off
/// mid-air, signalling late leaves a viewport-sized backing store on the
/// compositor for nothing.
const BURST_MS: u32 = 3600;
const TEARDOWN_MS: u32 = BURST_MS + 200;

#[cfg(feature = "web")]
fn start() {
    ffi::confetti_start(CANVAS_SELECTOR, ORIGIN_SELECTOR, BURST_MS as f64);
}
#[cfg(not(feature = "web"))]
fn start() {}

#[cfg(feature = "web")]
fn stop() {
    ffi::confetti_stop();
}
#[cfg(not(feature = "web"))]
fn stop() {}

/// `on_done` fires once the last piece has landed, for the caller to unmount
/// with. The burst does not unmount itself: the caller decides when it is
/// over, which is what lets it outlive the modal it erupted from.
#[component]
pub fn Confetti(on_done: EventHandler<()>) -> Element {
    // Effects never run during SSR, so this only schedules on the client -
    // the same reason `Countdown` can drive itself off a timer. It also runs
    // after the commit, so `.modal-content` is already in the document.
    use_effect(move || {
        start();
        spawn(async move {
            TimeoutFuture::new(TEARDOWN_MS).await;
            on_done.call(());
        });
    });

    // Only reached by a real unmount - navigating away, or the caller
    // responding to `on_done`. Either way the rAF loop goes with the node.
    use_drop(stop);

    rsx! {
        canvas {
            id: "confetti-canvas",
            class: "confetti-canvas",
            "aria-hidden": "true",
        }
    }
}
