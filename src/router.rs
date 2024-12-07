use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct History<P> {
    root: P,
    stack: Vec<P>,
}

impl<P> History<P> {
    pub fn new(root: P) -> Self {
        History {
            root,
            stack: vec![],
        }
    }

    pub fn push(&mut self, page: P) {
        self.stack.push(page);
    }

    pub fn len(&self) -> usize {
        self.stack.len() + 1
    }

    pub fn last(&self) -> &P {
        self.stack.last().unwrap_or(&self.root)
    }

    pub fn pop(&mut self) {
        self.stack.pop();
    }
}

pub struct Router<P>
where
    P: Clone,
{
    history: RefCell<History<Rc<P>>>,
    new_page: Cell<Option<Rc<P>>>,
}

impl<P> Router<P>
where
    P: Clone,
{
    pub fn new(initial: P) -> Self {
        Router {
            history: RefCell::new(History::new(Rc::new(initial))),
            new_page: Cell::new(None),
        }
    }

    pub fn push(&self, page: P) {
        let page = Rc::new(page);
        self.history.borrow_mut().push(page.clone());
        self.new_page.replace(Some(page));
       
    }

    pub fn pop(&self) {
        let mut history = self.history.borrow_mut();
        if history.len() > 1 {
            history.pop();
            self.new_page.replace(Some(history.last().clone()));
        }
    }

    pub fn poll_change(&self) -> Option<Rc<P>> {
        self.new_page.take()
    }

    pub fn can_go_back(&self) -> bool {
        self.history.borrow().len() > 1
    }
}
