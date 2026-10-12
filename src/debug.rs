//! Hooks for a debugger, such as the one in Earth Studio.
//!
//! An [`Interpreter`](crate::Interpreter) with a [`Debugger`] asks it before
//! every statement whether to pause there. When it should, the debugger is
//! handed a [`Snapshot`] of the program at that point (the line, the calls
//! that led there, the loops it is in and the variables it can see) and the
//! program waits until [`Debugger::pause`] returns.

use std::path::Path;

/// Decides where a program stops, and shows it while it is stopped.
pub trait Debugger {
    /// Called before every statement with its line (1-based), the module
    /// file it is in (`None` for the main program) and how many function
    /// calls deep it is (0 at the top level). Must be cheap.
    fn should_pause(&mut self, line: usize, file: Option<&Path>, depth: usize) -> bool;

    /// Called where `should_pause` said yes. Blocks until the user goes on;
    /// returns `false` to stop the program.
    fn pause(&mut self, snapshot: Snapshot) -> bool;
}

/// The state of a paused program.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub line: usize,
    /// The module file the line is in; `None` for the main program.
    pub file: Option<String>,
    /// The function calls that led here, outermost first. The first frame
    /// is the program itself, with an empty name.
    pub frames: Vec<Frame>,
    /// The loops the line is in, outermost first.
    pub loops: Vec<LoopTurn>,
    /// The variables the line can see, innermost scope first.
    pub variables: Vec<Variable>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The function's name; empty for the program itself.
    pub function: String,
    /// The line the frame is at: where it called the next frame, or, for
    /// the innermost one, the paused line.
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LoopTurn {
    /// The line of `her` or `iken`.
    pub line: usize,
    /// Which turn this is, from 1.
    pub turn: usize,
    /// How many turns a `her` loop has; `iken` loops do not know.
    pub total: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    pub name: String,
    /// The value as the program would print it, shortened if long.
    pub value: String,
    /// The type, as `tür` names it.
    pub type_name: String,
    /// Defined at the top level of the program rather than in a function
    /// or block.
    pub global: bool,
}
