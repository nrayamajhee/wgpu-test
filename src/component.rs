use crate::dom_factory::{create_el, create_shadow, window};
use maud::Markup;
use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Element, Event, HtmlElement, HtmlStyleElement, ShadowRoot};

type Fragment<T> = HashMap<String, Box<dyn Fn(&T) -> Markup>>;
type Attribute<T> = HashMap<String, Box<dyn Fn(&T) -> String>>;
type Handlers<T> = HashMap<String, Rc<(Box<dyn Fn(&T, Event) -> Option<T>>, Vec<String>)>>;
type Closures = HashMap<String, Closure<dyn FnMut(Event)>>;
type DirtyFragments = Rc<RefCell<Vec<String>>>;

pub struct Component<T> {
  state: Rc<RefCell<T>>,
  el: Element,
  dirty_fragments: DirtyFragments,
  fragments: Fragment<T>,
  attributes: Attribute<T>,
  closures: Closures,
}

impl<T: Clone + 'static> Component<T> {
  pub fn new(name: &str, state: T) -> Self {
    let (el, _) = create_shadow(name);
    let fragments = HashMap::new();
    let attributes = HashMap::new();
    let closures = HashMap::new();
    let dirty_fragments = Rc::new(RefCell::new(Vec::new()));
    let state = Rc::new(RefCell::new(state));
    Self {
      state,
      el,
      closures,
      fragments,
      attributes,
      dirty_fragments,
    }
  }
  pub fn markup(self, markup: impl Fn(&T) -> Markup) -> Self {
    self
      .shadow_root()
      .set_inner_html(markup(&self.state.borrow()).into_string().as_str());
    self
  }
  pub fn style(self, style: &str) -> Self {
    let style_el = create_el("style").dyn_into::<HtmlStyleElement>().unwrap();
    style_el.set_type("text/css");
    style_el.set_inner_html(&style);
    self.shadow_root().append_child(&style_el).unwrap();
    self
  }
  pub fn fragment(mut self, name: &str, fragment: impl Fn(&T) -> Markup + 'static) -> Self {
    self.fragments.insert(name.to_string(), Box::new(fragment));
    self.dirty_fragments.borrow_mut().push(name.to_string());
    self
  }
  pub fn bind(mut self, attribute: &str, function: impl Fn(&T) -> String + 'static) -> Self {
    self
      .attributes
      .insert(attribute.to_string(), Box::new(function));
    self
      .dirty_fragments
      .borrow_mut()
      .push(attribute.to_string());
    self
  }
  pub fn handler(
    mut self,
    name: &str,
    handler: impl Fn(&T, Event) -> Option<T> + 'static,
    deps: &[&str],
  ) -> Self {
    let s = self.state.clone();
    let df = self.dirty_fragments.clone();
    let deps: Vec<String> = deps.iter().map(|e| e.to_string()).collect();
    self.closures.insert(
      name.to_string(),
      Closure::wrap(Box::new(move |e| {
        let mut dirty_fragments = df.borrow_mut();
        let new_state = handler(&s.borrow(), e);
        if let Some(state) = new_state {
          *s.borrow_mut() = state;
        }
        dirty_fragments.extend_from_slice(&deps[..]);
      }) as Box<dyn FnMut(Event)>),
    );
    self
  }
  pub fn build(mut self) -> Self {
    self.update_frags(None);
    self.add_events(None);
    self
  }
  fn add_events(&self, root: Option<&str>) {
    let parent = if let Some(root) = root {
      format!("[data-fragment=\"{}\"]", root)
    } else {
      "".to_string()
    };
    let els = self
      .shadow_root()
      .query_selector_all(&format!("{} *[data-on]", parent))
      .unwrap();
    for i in 0..els.length() {
      let el = els.get(i).unwrap().dyn_into::<HtmlElement>().unwrap();
      let dataset = el.dataset();
      let event_type = dataset.get("on").unwrap();
      let event_handler = dataset.get("handle").unwrap();
      if let Some(closure) = self.closures.get(&event_handler) {
        el.add_event_listener_with_callback(&event_type, closure.as_ref().unchecked_ref())
          .unwrap();
      } else {
        log::error!(
          "Event handler \"{}\" doesn't exit in the component",
          event_handler
        );
      }
    }
  }
  fn update_frags(&self, root: Option<&str>) {
    let mut set: HashSet<String> = HashSet::new();
    let mut old_len = set.len();
    loop {
      let parent = if let Some(root) = root {
        format!("[data-fragment=\"{}\"]", root)
      } else {
        "".to_string()
      };
      let els = self
        .shadow_root()
        .query_selector_all(&format!("{} *[data-fragment]", parent))
        .unwrap();
      for i in 0..els.length() {
        let el = els.get(i).unwrap().dyn_into::<HtmlElement>().unwrap();
        let dataset = el.dataset();
        let name = dataset.get("fragment").unwrap();
        let data_id = dataset.get("uuid");
        let markup = self.fragments.get(&name).unwrap();
        if data_id == None || !set.contains(&data_id.unwrap()) {
          let uuid = window().crypto().unwrap().random_uuid();
          el.set_attribute("data-uuid", &uuid).unwrap();
          set.insert(uuid);
          el.set_inner_html(markup(&self.state.borrow()).into_string().as_str());
        }
      }
      if set.len() == old_len {
        break;
      } else {
        old_len = set.len();
      }
    }
  }
  pub fn update(&self) {
    for each in self.dirty_fragments.borrow().iter() {
      if self.fragments.contains_key(each) {
        let els = self
          .shadow_root()
          .query_selector_all(&format!("[data-fragment={}]", each))
          .unwrap();
        let markup = self.fragments.get(each).unwrap();
        for i in 0..els.length() {
          let el = els.get(i).unwrap().dyn_into::<HtmlElement>().unwrap();
          el.set_inner_html(markup(&self.state.borrow()).into_string().as_str());
          self.update_frags(Some(each));
          self.add_events(Some(each));
        }
      }
    }
    for each in self.dirty_fragments.borrow().iter() {
      if !self.fragments.contains_key(each) {
        log::info!("{}", each);
        let el = self
          .shadow_root()
          .query_selector(&format!("*[data-bind={}]", each))
          .unwrap()
          .unwrap();
        let el = el.dyn_into::<HtmlElement>().unwrap();
        let data = el.dataset();
        let attrib = data.get("attrib").unwrap();
        let func = data.get("bind").unwrap();
        let value = self.attributes.get(&func).unwrap();
        el.set_attribute(&attrib, &value(&self.state.borrow()))
          .unwrap();
      }
    }
    self.dirty_fragments.borrow_mut().clear();
  }
  pub fn element(&self) -> &Element {
    &self.el
  }
  pub fn shadow_root(&self) -> ShadowRoot {
    self.el.shadow_root().unwrap()
  }
  pub fn tag(&self, fragment: &str) {
    self.dirty_fragments.borrow_mut().push(fragment.to_string());
  }
  pub fn state(&self) -> T {
    self.state.borrow().clone()
  }
  pub fn set_state(&self, mode: T) {
    *self.state.borrow_mut() = mode;
  }
}
