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
    history: History<Rc<P>>,
    new_page: Option<Rc<P>>,
}

impl<P> Router<P>
where
    P: Clone,
{
    pub fn new(initial: P) -> Self {
        Router {
            history: History::new(Rc::new(initial)),
            new_page: None,
        }
    }

    pub fn push(&mut self, page: P) {
        let page = Rc::new(page);
        self.history.push(page.clone());
        self.new_page = Some(page);
    }

    pub fn pop(&mut self) {
        if self.history.len() > 1 {
            self.history.pop();
            self.new_page = Some(self.history.last().clone());
        }
    }

    pub fn poll_change(&mut self) -> Option<Rc<P>> {
        self.new_page.take()
    }

    pub fn can_go_back(&self) -> bool {
        self.history.len() > 1
    }
}
