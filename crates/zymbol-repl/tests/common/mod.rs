use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use zymbol_interpreter::{Interpreter, Value};

/// Result of executing one line through the harness.
pub struct LineResult {
    /// Characters printed via `>>` / `¶` during this call.
    pub output: String,
    /// Repr of the returned expression value (None for statements or Unit).
    #[allow(dead_code)] // test scaffolding: available for value-asserting tests
    pub value: Option<String>,
    /// Error message if execution failed.
    pub error: Option<String>,
}

/// Headless REPL test harness — no TTY required.
///
/// Wraps `Interpreter<Vec<u8>>` and provides per-call output capture,
/// mock input, and a simple history log.
pub struct ReplTestHarness {
    pub interpreter: Interpreter<Vec<u8>>,
    history: Vec<String>,
    input_queue: Rc<RefCell<VecDeque<String>>>,
}

impl Default for ReplTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplTestHarness {
    pub fn new() -> Self {
        let queue: Rc<RefCell<VecDeque<String>>> = Rc::new(RefCell::new(VecDeque::new()));
        let q2 = Rc::clone(&queue);

        let mut interpreter = Interpreter::with_output(Vec::<u8>::new());
        interpreter.set_input_fn(move || {
            Ok(q2.borrow_mut().pop_front().unwrap_or_default())
        });

        Self { interpreter, history: Vec::new(), input_queue: queue }
    }

    /// Queue lines that will be consumed one-by-one by `<<` input statements.
    pub fn queue_input(&self, lines: &[&str]) {
        let mut q = self.input_queue.borrow_mut();
        for &l in lines {
            q.push_back(l.to_string());
        }
    }

    /// Execute one line of Zymbol code and capture its output.
    pub fn run_line(&mut self, code: &str) -> LineResult {
        let result = self.interpreter.execute_line(code);
        let _ = self.interpreter.flush_output();

        // Drain the accumulated bytes (reset the vec for the next call).
        let bytes = std::mem::take(self.interpreter.writer_mut());
        let output = String::from_utf8_lossy(&bytes).into_owned();

        if !code.trim().is_empty() {
            self.history.push(code.to_string());
        }

        match result {
            Ok(Some(v)) if !matches!(v, Value::Unit) => LineResult {
                output,
                value: Some(self.interpreter.format_value_repr(&v)),
                error: None,
            },
            Ok(_) => LineResult { output, value: None, error: None },
            Err(e) => LineResult { output, value: None, error: Some(e.to_string()) },
        }
    }

    /// Convenience: run a line and return only its printed output.
    pub fn output(&mut self, code: &str) -> String {
        self.run_line(code).output
    }

    /// Convenience: run a line and return its expression-value repr.
    #[allow(dead_code)] // test scaffolding: available for value-asserting tests
    pub fn value(&mut self, code: &str) -> Option<String> {
        self.run_line(code).value
    }

    /// Convenience: run a line and return its error string.
    pub fn error(&mut self, code: &str) -> Option<String> {
        self.run_line(code).error
    }

    /// Lines executed so far (in order, excluding empty lines).
    pub fn history(&self) -> Vec<&str> {
        self.history.iter().map(|s| s.as_str()).collect()
    }

    /// Variables currently defined in the interpreter scope.
    pub fn variables(&self) -> Vec<(String, String)> {
        self.interpreter
            .list_variables()
            .into_iter()
            .map(|(name, val)| (name, self.interpreter.format_value(&val)))
            .collect()
    }
}
