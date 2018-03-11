mod tree;

use Context;
use Handler;
use Request;
use Result;
use Route;
use Tween;

use std::rc::Rc;

use hyper::Response;

#[derive(Clone)]
pub struct Router<S: Clone> {
    handlers: tree::Tree<S>,
    tweens: Vec<Tween<S>>,
}

impl<S: Clone + 'static> Router<S> {
    pub fn new() -> Router<S> {
        Router {
            handlers: tree::Tree::new(),
            tweens: vec![],
        }
    }

    pub fn get(mut self, pattern: &str, handler: Handler<S>) -> Router<S> {
        let trimmed = trim_path(pattern);
        let segments = trimmed.split("/");
        let mut current = 0;

        for segment in segments {
            if segment.starts_with(":") || segment.starts_with("*") {
                current = self.handlers.node_set_wildcard(
                    current,
                    String::from(segment)
                );
            } else {
                current = self.handlers.node_add_child(
                    current,
                    String::from(segment)
                );
            }
        }

        self.handlers.node_set_handler(current, handler);

        self
    }

    pub fn with(mut self, tween: Tween<S>) -> Router<S> {
        self.tweens.insert(0, tween);
        self
    }
}

pub fn handle<S: Clone + 'static>(router: &Router<S>, state: S, req: Rc<Request>) -> Result {
    let trimmed = trim_path(req.path());
    let segments = trimmed.split("/");
    let mut current = 0;
    let mut route = Route::new();

    for segment in segments {
        match router.handlers.node_get_child(current, String::from(segment)) {
            Some(child) => {
                current = *child;
            },
            None => {
                match router.handlers.node_get_wildcard(current) {
                    Some(wildcard) => {
                        current = wildcard.1;
                        if wildcard.0.starts_with(":") {
                            let mut wildcard_string = String::from(wildcard.0);
                            wildcard_string.remove(0);
                            route.params.insert(wildcard_string, String::from(segment));
                        } else {
                            break;
                        }
                    },
                    None => {
                        current = 0;
                        break;
                    },
                }
            },
        }
    }

    if let Some(handler) = router.handlers.node_get_handler(current) {
        let context = Context {
            state: state,
            route: Rc::new(route),
            req: req.clone(),
        };
        let chain = build_chain(router.tweens.clone(), Box::new(handler));
        return chain(context);
    }

    Ok(Response::new().with_body("404"))
}

fn build_chain<S: Clone + 'static>(
    mut tweens: Vec<Tween<S>>,
    next: Box<Fn(Context<S>) -> Result>,
) -> Box<Fn(Context<S>) -> Result> {
    if tweens.len() == 0 {
        return next;
    }

    let tween = tweens.pop().unwrap();
    let chain = build_chain(tweens.clone(), next);
    return Box::new(move |ctx: Context<S>| tween(ctx, &*chain));
}

fn trim_path(pattern: &str) -> String {
    let mut pattern_string = String::from(pattern);

    if pattern_string.starts_with("/") {
        pattern_string.remove(0);
    }

    if pattern_string.ends_with("/") {
        pattern_string.pop();
    }

    pattern_string
}
