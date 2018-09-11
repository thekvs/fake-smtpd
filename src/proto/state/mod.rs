#[derive(Debug, PartialEq)]
pub enum State {
    Invalid,
    Establish,
    Mail,
    Rcpt,
    Data,
    Done,
}

impl Default for State {
    fn default() -> Self {
        State::Invalid
    }
}
