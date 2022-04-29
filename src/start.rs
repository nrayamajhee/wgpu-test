use crate::dom_factory::{add_event_and_forget, add_style, body, on_animation_frame, window};
use crate::{Primitive, RayonWorkers, Renderer, Scene, Viewport };
use log::error;
use log::info;
use rayon::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use std::sync::{Arc, Mutex};

pub async fn start() -> Result<(), JsValue> {
    //Generate rayon thread-pool with the number of threads
    //given by window.navigator.hardwareConcurency
    //Spawn another web worker to run the multithreaded
    //part because blocking is not allowed in the main thread
    let window = window();
    let renderer = Renderer::new().await;
    let scene = Scene::new(&renderer.device());
    let viewport = Rc::new(Viewport::new());

    let plane_geo = scene.primitive(Primitive::Plane(None));
    let plane = scene.mesh("Plane", plane_geo);

    let circle_geo = scene.primitive(Primitive::Circle(None));
    let circle = scene.mesh("Circle", circle_geo);


    let shared_renderer = Rc::new(RefCell::new(renderer));
    let entities = Rc::new(vec![plane]);

    let r = shared_renderer.clone();
    let e = entities.clone();
    let v = viewport.clone();

    on_animation_frame(move || {
        r.borrow_mut()
            .render(e.as_ref(), v.as_ref())
            .unwrap_or_else(|_| error!("Render Error!"));
    });

    let r = shared_renderer.clone();
    add_event_and_forget(&window, "resize", move |_| {
        r.borrow_mut().resize();
    });

    add_style(
        "
        body {
            margin: 0;
        }
        canvas {
            display: block;
        }
    ",
    );
    body().append_child(shared_renderer.borrow().canvas());
    Ok(())
}
