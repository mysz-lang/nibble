use clap::ValueEnum;

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq)]
pub enum ResultType {
    Binary,
    Shared,
    Object,
}
