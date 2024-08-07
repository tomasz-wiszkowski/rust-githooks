#[allow(dead_code)]
pub enum Item {
    File(String),
    Commit(String),
    Link(String),
    Dir(String),
    Unknown(String),
}
