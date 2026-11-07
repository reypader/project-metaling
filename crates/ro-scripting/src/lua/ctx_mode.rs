/// How a hook call passes its context to Lua (the spike measures both).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CtxMode {
    /// A fresh table per call, with plain values and the two methods as scoped functions.
    #[default]
    Table,
    /// One scoped userdata per call that reads the context on access.
    Userdata,
}
