#[allow(dead_code)]
#[derive(Debug)]
pub enum Item {
    File(String),
    Commit(String),
    Link(String),
    Dir(String),
    Unknown(String),
}
