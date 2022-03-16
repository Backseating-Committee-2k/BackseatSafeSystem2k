pub struct Timer {
    get_ms_callback: Box<dyn FnMut() -> u64>,
}

impl Timer {
    pub fn new(get_ms_callback: impl FnMut() -> u64 + 'static) -> Self {
        Self {
            get_ms_callback: Box::new(get_ms_callback),
        }
    }

    pub fn get_ms_since_epoch(&mut self) -> u64 {
        (self.get_ms_callback)()
    }
}
