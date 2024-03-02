#[derive(Debug, PartialEq, Default)]
pub enum State {
    #[default]
    Invalid,
    Establish,
    Mail,
    Rcpt,
    Data,
    Done,
}
